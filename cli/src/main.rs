use std::str::FromStr;

use borsh::BorshDeserialize;
use program::Escrow;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    borsh::get_packed_len,
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    program_pack::Pack,
    pubkey::Pubkey,
    signature::Keypair,
    signer::keypair::read_keypair_file,
    signer::Signer,
    transaction::Transaction,
};
use structopt::StructOpt;

fn main() -> Result<(), Error> {
    let client =
        RpcClient::new_with_commitment("http://localhost:8899", CommitmentConfig::confirmed());
    match Command::from_args() {
        Command::Post(post) => do_post(&client, &post),
        Command::Take(take) => do_take(&client, &take),
    }
}

type Error = Box<dyn std::error::Error>;

#[derive(StructOpt)]
enum Command {
    Post(Post),
    Take(Take),
}

#[derive(StructOpt)]
struct Post {
    #[structopt(parse(try_from_str = read_keypair_file))]
    seller: Keypair,
    sell_account: Pubkey,
    sell_amount: u64,
    buy_account: Pubkey,
    buy_amount: u64,
}

#[derive(StructOpt)]
struct Take {
    #[structopt(parse(try_from_str = read_keypair_file))]
    taker: Keypair,
    taker_sell_account: Pubkey,
    taker_buy_account: Pubkey,
    escrow_account: Pubkey,
    #[structopt(short, long)]
    force: bool,
}

fn do_post(client: &RpcClient, post: &Post) -> Result<(), Error> {
    let sell_token_mint = get_token_mint(client, &post.sell_account)?;
    let escrow_account = Keypair::new();
    println!("Creating escrow account {}", escrow_account.pubkey());
    let token_account = Keypair::new();
    println!("Creating token account {}", token_account.pubkey());
    let instructions = [
        create_token_account_instruction(client, &post.seller.pubkey(), &token_account.pubkey())?,
        spl_token::instruction::initialize_account(
            &spl_token::ID,
            &token_account.pubkey(),
            &sell_token_mint,
            &post.seller.pubkey(),
        )?,
        spl_token::instruction::transfer(
            &spl_token::ID,
            &post.sell_account,
            &token_account.pubkey(),
            &post.seller.pubkey(),
            &[],
            post.sell_amount * LAMPORTS_PER_SOL,
        )?,
        create_escrow_instruction(
            client,
            &post.seller.pubkey(),
            &escrow_account.pubkey(),
            &program_id(),
        )?,
        post_trade_instruction(post, escrow_account.pubkey(), token_account.pubkey()),
    ];
    execute(
        client,
        &post.seller,
        &instructions,
        vec![&post.seller, &token_account, &escrow_account],
    )
}

fn program_id() -> Pubkey {
    Pubkey::from_str("77zL4LfjPjZbeCb8baAQ1pDvcWxNKxDFcVoJz5cxSFCv").unwrap()
}

fn get_token_mint(client: &RpcClient, token: &Pubkey) -> Result<Pubkey, Error> {
    let account = client.get_account(token)?;
    let account_info = spl_token::state::Account::unpack(&account.data)?;
    Ok(account_info.mint)
}

fn create_token_account_instruction(
    client: &RpcClient,
    seller: &Pubkey,
    token_account: &Pubkey,
) -> Result<Instruction, Error> {
    let space = spl_token::state::Account::LEN;
    let rent = client.get_minimum_balance_for_rent_exemption(space)?;
    Ok(solana_sdk::system_instruction::create_account(
        seller,
        token_account,
        rent,
        space as u64,
        &spl_token::ID,
    ))
}

fn create_escrow_instruction(
    client: &RpcClient,
    seller: &Pubkey,
    escrow_account: &Pubkey,
    program_id: &Pubkey,
) -> Result<Instruction, Error> {
    let space = get_packed_len::<program::Escrow>();
    let rent = client.get_minimum_balance_for_rent_exemption(space)?;
    Ok(solana_sdk::system_instruction::create_account(
        seller,
        escrow_account,
        rent,
        space as u64,
        program_id,
    ))
}

fn post_trade_instruction(
    post: &Post,
    escrow_account: Pubkey,
    token_account: Pubkey,
) -> Instruction {
    Instruction::new_with_borsh(
        program_id(),
        &program::Instruction::Post {
            buy_amount: post.buy_amount * LAMPORTS_PER_SOL,
        },
        vec![
            AccountMeta::new_readonly(post.seller.pubkey(), true),
            AccountMeta::new(token_account, false),
            AccountMeta::new_readonly(post.buy_account, false),
            AccountMeta::new(escrow_account, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ],
    )
}

fn execute(
    client: &RpcClient,
    payer: &Keypair,
    instructions: &[Instruction],
    signers: Vec<&Keypair>,
) -> Result<(), Error> {
    let blockhash = client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    client.send_and_confirm_transaction(&transaction)?;
    Ok(())
}

fn do_take(client: &RpcClient, take: &Take) -> Result<(), Error> {
    let escrow =
        Escrow::deserialize(&mut client.get_account(&take.escrow_account)?.data.as_slice())?;
    let buy_amount = get_token_amount(client, &escrow.token_account)?;
    if !take.force && !confirm_with_user(client, &escrow, buy_amount)? {
        return Err("Trade aborted".into());
    }
    let (pda, _) = Pubkey::find_program_address(&[program::ESCROW_SEED], &program_id());
    let instruction = take_trade_instruction(take, &escrow, buy_amount, pda);
    execute(client, &take.taker, &[instruction], vec![&take.taker])
}

fn get_token_amount(client: &RpcClient, token: &Pubkey) -> Result<u64, Error> {
    let account = client.get_account(token)?;
    let account_info = spl_token::state::Account::unpack(&account.data)?;
    Ok(account_info.amount)
}

fn confirm_with_user(client: &RpcClient, escrow: &Escrow, buy_amount: u64) -> Result<bool, Error> {
    let sell_token = get_token_mint(client, &escrow.poster_buy_account)?;
    let buy_token = get_token_mint(client, &escrow.token_account)?;
    println!("Preparing to do trade:");
    println!("  sell {} of {}", escrow.buy_amount, sell_token);
    println!("  buy {} of {}", buy_amount, buy_token);
    println!("  from user {}", escrow.poster);
    let answer = question::Question::new("Are you sure you want to continue?")
        .yes_no()
        .until_acceptable()
        .ask()
        .ok_or("Could not answer confirmation question")?;
    Ok(answer == question::Answer::YES)
}

fn take_trade_instruction(
    take: &Take,
    escrow: &Escrow,
    buy_amount: u64,
    pda: Pubkey,
) -> Instruction {
    Instruction::new_with_borsh(
        program_id(),
        &program::Instruction::Take {
            buy_amount,
            sell_amount: escrow.buy_amount,
        },
        vec![
            AccountMeta::new_readonly(take.taker.pubkey(), true),
            AccountMeta::new(take.taker_sell_account, false),
            AccountMeta::new(take.taker_buy_account, false),
            AccountMeta::new(escrow.token_account, false),
            AccountMeta::new(escrow.poster, false),
            AccountMeta::new(escrow.poster_buy_account, false),
            AccountMeta::new(take.escrow_account, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(pda, false),
        ],
    )
}
