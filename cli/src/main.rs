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
    }
}

type Error = Box<dyn std::error::Error>;

#[derive(StructOpt)]
enum Command {
    Post(Post),
}

#[derive(StructOpt)]
struct Post {
    program_id: Pubkey,
    token_x_mint: Pubkey,
    #[structopt(parse(try_from_str = read_keypair_file))]
    seller: Keypair,
    sell_account: Pubkey,
    sell_amount: u64,
    receive_account: Pubkey,
    receive_amount: u64,
}

// TODO TOKEN_PROGRAM_ID? Same as post.program_id?
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
            amount: post.receive_amount,
        },
        vec![
            AccountMeta::new_readonly(post.seller.pubkey(), true),
            AccountMeta::new(token_account, false),
            AccountMeta::new_readonly(post.receive_account, false),
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
