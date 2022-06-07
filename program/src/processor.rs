use crate::instruction::Instruction;
use crate::{error::Error, state::Escrow};

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program::invoke;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction =
        Instruction::try_from_slice(instruction_data).map_err(|_| Error::InvalidInstruction)?;

    match instruction {
        Instruction::Post { amount } => process_post(program_id, accounts, amount),
    }
}

fn process_post(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    msg!("Instruction: Post");

    //
    // deserialize accounts info
    //
    let mut accounts_iter = accounts.iter();

    let seller = next_account_info(&mut accounts_iter)?;
    if !seller.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let trade_account = next_account_info(&mut accounts_iter)?;
    if *trade_account.owner != spl_token::id() {
        // TODO this check not necessary because transfer would fail anyway?
        return Err(ProgramError::IncorrectProgramId);
    }

    let receive_account = next_account_info(&mut accounts_iter)?;
    if *receive_account.owner != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let escrow_account = next_account_info(&mut accounts_iter)?;

    if !Rent::get()?.is_exempt(escrow_account.lamports(), escrow_account.data_len()) {
        return Err(Error::NotRentExempt.into());
    }

    let token_program = next_account_info(&mut accounts_iter)?;

    //
    // set escrow info
    //

    let mut escrow_info = Escrow::try_from_slice(&escrow_account.try_borrow_data()?)
        .map_err(|_| Error::InvalidEscrowAccount)?;
    if escrow_info.is_initialized {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    escrow_info.is_initialized = true;
    escrow_info.seller = *seller.key;
    escrow_info.seller_trade_account = *trade_account.key;
    escrow_info.seller_receive_account = *receive_account.key;
    escrow_info.amount = amount;

    escrow_info.serialize(&mut *escrow_account.try_borrow_mut_data()?)?;

    //
    // transfer ownsership of trade account to PDA
    //
    let (pda, _bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);
    let owner_change_instruction = spl_token::instruction::set_authority(
        token_program.key,
        trade_account.key,
        Some(&pda),
        spl_token::instruction::AuthorityType::AccountOwner,
        seller.key,
        &[seller.key],
    )?;

    msg!("Calling the token program to transfer token account ownership...");
    invoke(
        &owner_change_instruction,
        &[trade_account.clone(), seller.clone(), token_program.clone()],
    )?;

    Ok(())
}
