use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub enum Instruction {
    /// Starts the trade by creating and populating an escrow account and transferring ownership of the given temp token account to the PDA
    ///
    /// Accounts expected:
    ///
    /// 0. `[signer]` The account of the poster: the person posting the trade
    /// 1. `[writable]` Temporary token account that should be created prior to this instruction and owned by the poster
    /// 2. `[]` The poster's token account for the token they will receive should the trade go through
    /// 3. `[writable]` The escrow account, it will hold all necessary info about the trade.
    /// 4. `[]` The token program
    Post {
        /// Amount party A expects to receive of token Y
        buy_amount: u64,
    },

    /// Takes a trade that a seller has Posted
    ///
    /// Accounts expected:
    ///
    /// 0. `[signer]` The account of the taker (person taking the trade)
    /// 1. `[writable]` The taker's token account for the token they send
    /// 2. `[writable]` The taker's token account for the token they will receive should the trade go through
    /// 3. `[writable]` The PDA's temp token account to get tokens from and eventually close
    /// 4. `[writable]` The poster's main account to send their rent fees to
    /// 5. `[writable]` The poster's token account that will receive tokens
    /// 6. `[writable]` The escrow account holding the escrow info
    /// 7. `[]` The token program
    /// 8. `[]` The PDA account
    Take { buy_amount: u64, sell_amount: u64 },
}
