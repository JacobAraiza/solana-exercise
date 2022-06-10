use std::str::FromStr;

use borsh::BorshDeserialize;
use program::Escrow;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    borsh::get_packed_len,
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    program_pack::Pack,
    pubkey::Pubkey,
    signature::Keypair,
    signer::keypair::read_keypair_file,
    signer::Signer,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use structopt::StructOpt;

fn main() -> Result<(), Error> {
    let client =
        RpcClient::new_with_commitment("http://localhost:8899", CommitmentConfig::confirmed());
    match Command::from_args() {
        Command::Create(create) => do_create_fee_account(&client, &create),
        Command::Post(post) => do_post(&client, &post),
        Command::Take(take) => do_take(&client, &take),
        Command::Cancel(cancel) => do_cancel(&client, &cancel),
    }
}

type Error = Box<dyn std::error::Error>;

#[derive(StructOpt)]
enum Command {
    Create(Create),
    Post(Post),
    Take(Take),
    Cancel(Cancel),
}

#[derive(StructOpt)]
struct Create {
    #[structopt(parse(try_from_str = read_keypair_file))]
    administrator: Keypair,
}

#[derive(StructOpt)]
struct Post {
    #[structopt(parse(try_from_str = read_keypair_file))]
    poster: Keypair,
    sell_token: Pubkey,
    sell_amount: u64,
    buy_token: Pubkey,
    buy_amount: u64,
}

#[derive(StructOpt)]
struct Take {
    #[structopt(parse(try_from_str = read_keypair_file))]
    taker: Keypair,
    escrow_account: Pubkey,
    #[structopt(short, long)]
    force: bool,
}

#[derive(StructOpt)]
struct Cancel {
    #[structopt(parse(try_from_str = read_keypair_file))]
    poster: Keypair,
    escrow_account: Pubkey,
}

fn do_create_fee_account(client: &RpcClient, create: &Create) -> Result<(), Error> {
    let fee_account = Keypair::new();
    println!("Making new fee account {}", fee_account.pubkey());
    let rent = client.get_minimum_balance_for_rent_exemption(0)?;
    execute(
        client,
        &create.administrator,
        &[solana_sdk::system_instruction::create_account(
            &create.administrator.pubkey(),
            &fee_account.pubkey(),
            rent,
            0,
            &program_id(),
        )],
        vec![&create.administrator, &fee_account],
    )
}

///
/// Post trade
///

fn do_post(client: &RpcClient, post: &Post) -> Result<(), Error> {
    let sell_account = get_associated_token_address(&post.poster.pubkey(), &post.sell_token);
    let buy_account = get_associated_token_address(&post.poster.pubkey(), &post.buy_token);
    let escrow_account = Keypair::new();
    let token_account = Keypair::new();
    println!("Creating escrow account {}", escrow_account.pubkey());
    println!("Creating token account {}", token_account.pubkey());
    println!("Using sell Associated Token Account {}", sell_account);
    println!("Using buy Associated Token Account {}", buy_account);

    let mut instructions = Vec::new();
    add_associated_token_account(
        client,
        &buy_account,
        &post.poster.pubkey(),
        &post.poster.pubkey(),
        &post.buy_token,
        &mut instructions,
    )?;
    instructions.extend_from_slice(&[
        create_token_account_instruction(client, &post.poster.pubkey(), &token_account.pubkey())?,
        spl_token::instruction::initialize_account(
            &spl_token::ID,
            &token_account.pubkey(),
            &post.sell_token,
            &post.poster.pubkey(),
        )?,
        spl_token::instruction::transfer(
            &spl_token::ID,
            &sell_account,
            &token_account.pubkey(),
            &post.poster.pubkey(),
            &[],
            post.sell_amount * LAMPORTS_PER_SOL,
        )?,
        create_escrow_instruction(
            client,
            &post.poster.pubkey(),
            &escrow_account.pubkey(),
            &program_id(),
        )?,
        post_trade_instruction(
            post,
            buy_account,
            escrow_account.pubkey(),
            token_account.pubkey(),
        ),
    ]);
    execute(
        client,
        &post.poster,
        &instructions,
        vec![&post.poster, &token_account, &escrow_account],
    )
}

fn get_token_mint(client: &RpcClient, token: &Pubkey) -> Result<Pubkey, Error> {
    let account = client.get_account(token)?;
    let account_info = spl_token::state::Account::unpack(&account.data)?;
    Ok(account_info.mint)
}

fn create_token_account_instruction(
    client: &RpcClient,
    poster: &Pubkey,
    token_account: &Pubkey,
) -> Result<Instruction, Error> {
    let space = spl_token::state::Account::LEN;
    let rent = client.get_minimum_balance_for_rent_exemption(space)?;
    Ok(solana_sdk::system_instruction::create_account(
        poster,
        token_account,
        rent,
        space as u64,
        &spl_token::ID,
    ))
}

fn create_escrow_instruction(
    client: &RpcClient,
    poster: &Pubkey,
    escrow_account: &Pubkey,
    program_id: &Pubkey,
) -> Result<Instruction, Error> {
    let space = get_packed_len::<program::Escrow>();
    let rent = client.get_minimum_balance_for_rent_exemption(space)?;
    Ok(solana_sdk::system_instruction::create_account(
        poster,
        escrow_account,
        rent,
        space as u64,
        program_id,
    ))
}

fn post_trade_instruction(
    post: &Post,
    buy_account: Pubkey,
    escrow_account: Pubkey,
    token_account: Pubkey,
) -> Instruction {
    Instruction::new_with_borsh(
        program_id(),
        &program::Instruction::Post {
            buy_amount: post.buy_amount * LAMPORTS_PER_SOL,
        },
        vec![
            AccountMeta::new_readonly(post.poster.pubkey(), true),
            AccountMeta::new(token_account, false),
            AccountMeta::new_readonly(buy_account, false),
            AccountMeta::new(escrow_account, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
            AccountMeta::new(program::fee_account_pubkey(), false),
        ],
    )
}

///
/// Take trade
///

fn do_take(client: &RpcClient, take: &Take) -> Result<(), Error> {
    let escrow =
        Escrow::deserialize(&mut client.get_account(&take.escrow_account)?.data.as_slice())?;
    let sell_token = get_token_mint(client, &escrow.poster_buy_account)?;
    let buy_token = get_token_mint(client, &escrow.token_account)?;
    let buy_amount = get_token_amount(client, &escrow.token_account)?;
    if !take.force && !confirm_with_user(&escrow, buy_amount, &sell_token, &buy_token)? {
        return Err("Trade aborted".into());
    }

    let taker_sell_account = get_associated_token_address(&take.taker.pubkey(), &sell_token);
    let taker_buy_account = get_associated_token_address(&take.taker.pubkey(), &buy_token);

    let mut instructions = Vec::new();
    add_associated_token_account(
        client,
        &taker_buy_account,
        &take.taker.pubkey(),
        &take.taker.pubkey(),
        &buy_token,
        &mut instructions,
    )?;
    let (pda, _) = Pubkey::find_program_address(&[program::ESCROW_SEED], &program_id());
    instructions.push(take_trade_instruction(
        take,
        &escrow,
        taker_sell_account,
        taker_buy_account,
        buy_amount,
        pda,
    ));

    execute(client, &take.taker, &instructions, vec![&take.taker])
}

fn get_token_amount(client: &RpcClient, token: &Pubkey) -> Result<u64, Error> {
    let account = client.get_account(token)?;
    let account_info = spl_token::state::Account::unpack(&account.data)?;
    Ok(account_info.amount)
}

fn confirm_with_user(
    escrow: &Escrow,
    buy_amount: u64,
    sell_token: &Pubkey,
    buy_token: &Pubkey,
) -> Result<bool, Error> {
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
    taker_sell_account: Pubkey,
    taker_buy_account: Pubkey,
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
            AccountMeta::new(taker_sell_account, false),
            AccountMeta::new(taker_buy_account, false),
            AccountMeta::new(escrow.token_account, false),
            AccountMeta::new(escrow.poster, false),
            AccountMeta::new(escrow.poster_buy_account, false),
            AccountMeta::new(take.escrow_account, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(pda, false),
        ],
    )
}

//
// Cancel existing trade
//

fn do_cancel(client: &RpcClient, cancel: &Cancel) -> Result<(), Error> {
    let escrow_info =
        Escrow::deserialize(&mut client.get_account(&cancel.escrow_account)?.data.as_ref())?;
    let sell_token = get_token_mint(client, &escrow_info.token_account)?;
    let refund_account = get_associated_token_address(&cancel.poster.pubkey(), &sell_token);
    let instructions = [cancel_trade_instruction(
        cancel,
        escrow_info.token_account,
        refund_account,
    )];
    execute(client, &cancel.poster, &instructions, vec![&cancel.poster])
}

fn cancel_trade_instruction(
    cancel: &Cancel,
    token_account: Pubkey,
    refund_account: Pubkey,
) -> Instruction {
    let (pda, _) = Pubkey::find_program_address(&[program::ESCROW_SEED], &program_id());
    Instruction::new_with_borsh(
        program_id(),
        &program::Instruction::Cancel {},
        vec![
            AccountMeta::new_readonly(cancel.poster.pubkey(), true),
            AccountMeta::new(token_account, false),
            AccountMeta::new(cancel.escrow_account, false),
            AccountMeta::new(refund_account, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(pda, false),
        ],
    )
}

//
// Common functions
//

fn program_id() -> Pubkey {
    Pubkey::from_str("77zL4LfjPjZbeCb8baAQ1pDvcWxNKxDFcVoJz5cxSFCv").unwrap()
}

fn add_associated_token_account(
    client: &RpcClient,
    associated_account_address: &Pubkey,
    funding_address: &Pubkey,
    wallet_address: &Pubkey,
    spl_token_mint_address: &Pubkey,
    instructions: &mut Vec<Instruction>,
) -> Result<(), Error> {
    let config = CommitmentConfig {
        commitment: CommitmentLevel::Processed, // TODO is this the right way to check if account exists?
    };
    if client
        .get_account_with_commitment(associated_account_address, config)?
        .value
        .is_some()
    {
        return Ok(());
    }
    instructions.push(
        spl_associated_token_account::instruction::create_associated_token_account(
            funding_address,
            wallet_address,
            spl_token_mint_address,
        ),
    );
    Ok(())
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
