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
    program_id: Pubkey,
    token_x_mint: Pubkey,
    #[structopt(parse(try_from_str = read_keypair_file))]
    seller: Keypair,
    sell_account: Pubkey,
    sell_amount: u64,
    buy_account: Pubkey,
    buy_amount: u64,
}

#[derive(StructOpt)]
struct Take {
    program_id: Pubkey,
    #[structopt(parse(try_from_str = read_keypair_file))]
    taker: Keypair,
    taker_sell_account: Pubkey,
    sell_amount: u64,
    taker_buy_account: Pubkey,
    buy_amount: u64,
    poster: Pubkey,
    poster_buy_account: Pubkey,
    token_account: Pubkey,
    escrow_account: Pubkey,
}

fn do_post(client: &RpcClient, post: &Post) -> Result<(), Error> {
    let escrow_account = Keypair::new();
    println!("Creating escrow account {}", escrow_account.pubkey());
    let token_account = Keypair::new();
    println!("Creating token account {}", token_account.pubkey());
    let instructions = [
        create_token_account_instruction(client, &post.seller.pubkey(), &token_account.pubkey())?,
        spl_token::instruction::initialize_account(
            &spl_token::ID,
            &token_account.pubkey(),
            &post.token_x_mint,
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
            &post.program_id,
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
        post.program_id,
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
    let (pda, _) = Pubkey::find_program_address(&[&b"escrow"[..]], &take.program_id);
    let instruction = take_trade_instruction(take, pda);
    execute(client, &take.taker, &[instruction], vec![&take.taker])
}

fn take_trade_instruction(take: &Take, pda: Pubkey) -> Instruction {
    Instruction::new_with_borsh(
        take.program_id,
        &program::Instruction::Take {
            buy_amount: take.buy_amount * LAMPORTS_PER_SOL,
            sell_amount: take.sell_amount * LAMPORTS_PER_SOL,
        },
        vec![
            AccountMeta::new_readonly(take.taker.pubkey(), true),
            AccountMeta::new(take.taker_sell_account, false),
            AccountMeta::new(take.taker_buy_account, false),
            AccountMeta::new(take.token_account, false),
            AccountMeta::new(take.poster, false),
            AccountMeta::new(take.poster_buy_account, false),
            AccountMeta::new(take.escrow_account, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(pda, false),
        ],
    )
}
