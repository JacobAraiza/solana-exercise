use crate::instruction::Instruction;
use crate::{error::Error, state::Escrow};

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
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
        Instruction::Post { buy_amount } => process_post(program_id, accounts, buy_amount),
        Instruction::Take {
            buy_amount,
            sell_amount,
        } => process_take(program_id, accounts, buy_amount, sell_amount),
    }
}

fn process_post(program_id: &Pubkey, accounts: &[AccountInfo], buy_amount: u64) -> ProgramResult {
    msg!("Instruction: Post");

    //
    // deserialize accounts info
    //
    let mut accounts_iter = accounts.iter();

    let poster = next_account_info(&mut accounts_iter)?;
    if !poster.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let token_account = next_account_info(&mut accounts_iter)?;
    if *token_account.owner != spl_token::id() {
        // TODO this check not necessary because transfer would fail anyway?
        return Err(ProgramError::IncorrectProgramId);
    }

    let buy_account = next_account_info(&mut accounts_iter)?;
    if *buy_account.owner != spl_token::id() {
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
    escrow_info.poster = *poster.key;
    escrow_info.token_account = *token_account.key;
    escrow_info.poster_buy_account = *buy_account.key;
    escrow_info.buy_amount = buy_amount;

    escrow_info.serialize(&mut *escrow_account.try_borrow_mut_data()?)?;

    //
    // transfer ownsership of trade account to PDA
    //
    let (pda, _bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);
    let owner_change_instruction = spl_token::instruction::set_authority(
        token_program.key,
        token_account.key,
        Some(&pda),
        spl_token::instruction::AuthorityType::AccountOwner,
        poster.key,
        &[poster.key],
    )?;

    msg!("Calling the token program to transfer token account ownership...");
    invoke(
        &owner_change_instruction,
        &[token_account.clone(), poster.clone(), token_program.clone()],
    )?;

    Ok(())
}

fn process_take(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    buy_amount: u64,
    sell_amount: u64,
) -> ProgramResult {
    msg!("Instruction: Take");

    //
    // deserialize accounts info
    //
    msg!("Deserializing accounts");
    let mut accounts_iter = accounts.iter();

    let taker = next_account_info(&mut accounts_iter)?;
    if !taker.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let taker_sell_account = next_account_info(&mut accounts_iter)?;
    let taker_buy_account = next_account_info(&mut accounts_iter)?;
    let token_account = next_account_info(&mut accounts_iter)?;
    let poster = next_account_info(&mut accounts_iter)?;
    let poster_buy_account = next_account_info(&mut accounts_iter)?;
    let escrow_account = next_account_info(&mut accounts_iter)?;
    let token_program = next_account_info(&mut accounts_iter)?;
    let pda_account = next_account_info(&mut accounts_iter)?;

    //
    // Deserialize token account info
    //
    msg!("Deserializing token account");
    let token_info = spl_token::state::Account::unpack(&token_account.try_borrow_data()?)?;
    if buy_amount != token_info.amount {
        return Err(Error::ExpectedAmountMismatch.into());
    }

    //
    // Deserialize escrow account info
    //
    msg!("Deserializing escrow info");
    let escrow_info = Escrow::deserialize(&mut escrow_account.try_borrow_data()?.as_ref())?;
    if escrow_info.token_account != *token_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    if escrow_info.poster != *poster.key {
        return Err(ProgramError::InvalidAccountData);
    }
    if escrow_info.poster_buy_account != *poster_buy_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    if escrow_info.buy_amount != sell_amount {
        return Err(Error::ExpectedAmountMismatch.into());
    }

    //
    // Send token Y amount from taker's to poster's account
    //
    msg!("Sending token Y from Taker to Poster");
    invoke(
        &spl_token::instruction::transfer(
            token_program.key,
            taker_sell_account.key,
            poster_buy_account.key,
            taker.key,
            &[taker.key],
            escrow_info.buy_amount,
        )?,
        &[
            taker_sell_account.clone(),
            poster_buy_account.clone(),
            taker.clone(),
            token_program.clone(),
        ],
    )?;

    //
    // Send token X amount from poster's to taker's account
    //
    msg!("Sending token X from Poster to Taker");
    let (pda, bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);
    invoke_signed(
        &spl_token::instruction::transfer(
            token_program.key,
            token_account.key,
            taker_buy_account.key,
            &pda,
            &[&pda],
            token_info.amount,
        )?,
        &[
            token_account.clone(),
            taker_buy_account.clone(),
            pda_account.clone(),
            token_program.clone(),
        ],
        &[&[&b"escrow"[..], &[bump_seed]]],
    )?;

    //
    // Close the token account
    //
    msg!("Closing token account");
    invoke_signed(
        &spl_token::instruction::close_account(
            token_program.key,
            token_account.key,
            poster.key,
            &pda,
            &[&pda],
        )?,
        &[
            token_account.clone(),
            poster.clone(),
            pda_account.clone(),
            token_program.clone(),
        ],
        &[&[&b"escrow"[..], &[bump_seed]]],
    )?;

    //
    // Close escrow account (returning rent to poster)
    //
    msg!("Closing escrow");
    **poster.lamports.borrow_mut() = poster
        .lamports()
        .checked_add(escrow_account.lamports())
        .ok_or(Error::AmountOverflow)?;
    **escrow_account.lamports.borrow_mut() = 0;
    *escrow_account.try_borrow_mut_data()? = &mut [];

    Ok(())
}
