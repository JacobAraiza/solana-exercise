## Environment Setup
1. Install Rust from https://rustup.rs/
2. Install Solana from https://docs.solana.com/cli/install-solana-cli-tools#use-solanas-install-tool

## Testing

### Initial Setup

- Start a test validator instance on http://localhost:8899 in another terminal, and leave that running:

```
solana-test-validator
```

- Create an account with som SOL if you don't have one already.

- Build an deploy the program for the first time (make a note of the program public key):

```
cargo build-bpf
cargo deploy PATH_TO_PROGRAM
```

- Edit the code in `./cli/src/main.rs:program_id()` to contain the public key from the step above.

- Create the account for the fees for trades to be payed into (make a note of the account public key):

```
cargo run -- create PATH_TO_YOUR_KEYPAIR
```

- Edit the code in `./program/src/processor:fee_account_pubkey()` to return the public key from the step above.

- Rebuild and deploy the program using the commands from before.

### Integration Test

- run `./script/run.sh`

## TODO
- unit testing
- pda_account not checked.
- get_associated_token_account are called with the wrong value, test with wallet.
- Should avoid using temp accounts and set authority, instead create PDA account and transfer balance.

- No mint pk on chain makes for difficult indexing.
- The CLI doesn't have an option for listing
- Delegation would simplify the process and limit escrow