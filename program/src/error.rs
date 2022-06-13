use num_derive::FromPrimitive;
use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum Error {
    #[error("Invalid Instruction")]
    InvalidInstruction,
    #[error("Escrow account is not rent exempt")]
    NotRentExempt,
    #[error("Escrow account data is missing or invalid")]
    InvalidEscrowAccount,
    #[error("Buyer and seller amounts do not match up")]
    ExpectedAmountMismatch,
    #[error("Lamport amount overflow")]
    AmountOverflow,
    #[error("Not enough lamports to pay for fee")]
    NotEnoughForFee,
    #[error("Not the right account to pay fee into")]
    IncorrectFeeAccount,
    #[error("Accounts supplied do not match information in escrow account")]
    DoesntMatchEscrow,
    #[error("Account is not a token account (not owned by token id)")]
    AccountNotToken,
    #[error("Incorrect PDA account")]
    IncorrectPDA,
}

impl From<Error> for ProgramError {
    fn from(error: Error) -> Self {
        ProgramError::Custom(error as u32)
    }
}

impl<T> DecodeError<T> for Error {
    fn type_of() -> &'static str {
        "TokenError"
    }
}
