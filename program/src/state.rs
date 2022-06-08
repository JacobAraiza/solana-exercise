use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Escrow {
    pub is_initialized: bool,
    pub poster: Pubkey,
    pub token_account: Pubkey,
    pub poster_buy_account: Pubkey,
    pub buy_amount: u64,
}
