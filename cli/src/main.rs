use solana_program::pubkey::Pubkey;
use structopt::StructOpt;

fn main() -> Result<(), Error> {
    let command = Command::from_args();
    Ok(())
}

type Error = Box<dyn std::error::Error>;

#[derive(StructOpt)]
enum Command {
    Post(Post),
    Cancel(Cancel),
    Take(Take),
}

#[derive(StructOpt)]
struct Post {
    seller: Pubkey,
    account: Pubkey,
    amount: u64,
}

#[derive(StructOpt)]
struct Cancel {
    trade: Pubkey,
}

#[derive(StructOpt)]
struct Take {
    buyer: Pubkey,
    trade: Pubkey,
    amount: u64,
}
