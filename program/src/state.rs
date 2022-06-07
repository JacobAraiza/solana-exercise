use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Escrow {
    pub is_initialized: bool,
    pub seller: Pubkey,
    pub seller_trade_account: Pubkey,
    pub seller_receive_account: Pubkey,
    pub amount: u64,
}
