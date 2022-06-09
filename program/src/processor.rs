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

pub const ESCROW_SEED: &[u8] = b"escrow";

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
        Instruction::Cancel {} => process_cancel(program_id, accounts),
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
    let (pda, _bump_seed) = Pubkey::find_program_address(&[ESCROW_SEED], program_id);
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
    // Send token X amount from token account to taker's account, then close account
    //
    msg!("Sending token X from Poster to Taker");
    transfer_and_close(
        program_id,
        token_program,
        token_account,
        taker_buy_account,
        poster,
        pda_account,
        token_info.amount,
    )?;

    //
    // Close escrow account (returning rent to poster)
    //
    close_escrow(escrow_account, poster)?;

    Ok(())
}

fn process_cancel(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    //
    // deserialize accounts info
    //
    msg!("Deserializing accounts");
    let mut accounts_iter = accounts.iter();

    let poster = next_account_info(&mut accounts_iter)?;
    if !poster.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let token_account = next_account_info(&mut accounts_iter)?;
    if *token_account.owner != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let escrow = next_account_info(&mut accounts_iter)?;
    let refund_account = next_account_info(&mut accounts_iter)?;
    let token_program = next_account_info(&mut accounts_iter)?;
    let pda_account = next_account_info(&mut accounts_iter)?;

    //
    // Deserialize token account info
    //
    msg!("Deserializing token account");
    let token_info = spl_token::state::Account::unpack(&token_account.try_borrow_data()?)?;

    //
    // Deserialize escrow account info
    //
    msg!("Deserializing escrow info");
    let escrow_info = Escrow::deserialize(&mut escrow.try_borrow_data()?.as_ref())?;
    if escrow_info.token_account != *token_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    if escrow_info.poster != *poster.key {
        return Err(ProgramError::InvalidAccountData);
    }

    //
    // Transfer authority of tokens account back to poster
    //
    msg!("Calling the token program to transfer token account ownership...");
    transfer_and_close(
        program_id,
        token_program,
        token_account,
        refund_account,
        poster,
        pda_account,
        token_info.amount,
    )?;

    //
    // Close escrow account
    //
    close_escrow(escrow, poster)?;

    Ok(())
}

fn transfer_and_close<'a>(
    program_id: &Pubkey,
    token_program: &AccountInfo<'a>,
    source_account: &AccountInfo<'a>,
    destination_account: &AccountInfo<'a>,
    poster: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    let (pda, bump_seed) = Pubkey::find_program_address(&[ESCROW_SEED], program_id);
    invoke_signed(
        &spl_token::instruction::transfer(
            token_program.key,
            source_account.key,
            destination_account.key,
            &pda,
            &[&pda],
            amount,
        )?,
        &[
            source_account.clone(),
            destination_account.clone(),
            pda_account.clone(),
            token_program.clone(),
        ],
        &[&[ESCROW_SEED, &[bump_seed]]],
    )?;
    invoke_signed(
        &spl_token::instruction::close_account(
            token_program.key,
            source_account.key,
            poster.key,
            &pda,
            &[&pda],
        )?,
        &[
            source_account.clone(),
            poster.clone(),
            pda_account.clone(),
            token_program.clone(),
        ],
        &[&[ESCROW_SEED, &[bump_seed]]],
    )?;
    Ok(())
}

fn close_escrow(escrow: &AccountInfo, poster: &AccountInfo) -> ProgramResult {
    msg!("Closing escrow account");
    **poster.lamports.borrow_mut() = poster
        .lamports()
        .checked_add(escrow.lamports())
        .ok_or(Error::AmountOverflow)?;
    **escrow.lamports.borrow_mut() = 0;
    *escrow.try_borrow_mut_data()? = &mut [];
    Ok(())
}
