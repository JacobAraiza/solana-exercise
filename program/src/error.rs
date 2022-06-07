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
