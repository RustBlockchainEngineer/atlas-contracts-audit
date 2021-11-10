//! Program state processor

use crate::constraints::*;
use crate::{
    curve::{
        base::{SwapCurve, CurveType},
        constant_product::ConstantProductCurve,
        calculator::{RoundDirection, TradeDirection, INITIAL_SWAP_POOL_AMOUNT},
        fees::Fees,
    },
    error::SwapError,
    instruction::{
        DepositAllTokenTypes, DepositSingleTokenTypeExactAmountIn, Initialize, Swap,
        SwapInstruction, WithdrawAllTokenTypes, WithdrawSingleTokenTypeExactAmountOut,SetGlobalState
    },
    state::{SwapState, SwapV1, SwapVersion, GlobalState},
};
use num_traits::FromPrimitive;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program::invoke,
    system_instruction,
    program_error::{PrintProgramError, ProgramError},
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
};
use std::convert::TryInto;
use std::str::FromStr;

/// Program state handler.
pub struct Processor {}
impl Processor {
    /// Unpacks a spl_token `Account`.
    pub fn unpack_token_account(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<spl_token::state::Account, SwapError> {
        if account_info.owner != token_program_id {
            Err(SwapError::IncorrectTokenProgramId)
        } else {
            spl_token::state::Account::unpack(&account_info.data.borrow())
                .map_err(|_| SwapError::ExpectedAccount)
        }
    }

    /// Unpacks a spl_token `Mint`.
    pub fn unpack_mint(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<spl_token::state::Mint, SwapError> {
        if account_info.owner != token_program_id {
            Err(SwapError::IncorrectTokenProgramId)
        } else {
            spl_token::state::Mint::unpack(&account_info.data.borrow())
                .map_err(|_| SwapError::ExpectedMint)
        }
    }

    /// Calculates the authority id by generating a program address.
    pub fn authority_id(
        program_id: &Pubkey,
        my_info: &Pubkey,
        nonce: u8,
    ) -> Result<Pubkey, SwapError> {
        Pubkey::create_program_address(&[&my_info.to_bytes()[..32], &[nonce]], program_id)
            .or(Err(SwapError::InvalidProgramAddress))
    }

    /// Issue a spl_token `Burn` instruction.
    pub fn token_burn<'a>(
        swap: &Pubkey,
        token_program: AccountInfo<'a>,
        burn_account: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let swap_bytes = swap.to_bytes();
        let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            authority.key,
            &[],
            amount,
        )?;

        invoke_signed(
            &ix,
            &[burn_account, mint, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `MintTo` instruction.
    pub fn token_mint_to<'a>(
        swap: &Pubkey,
        token_program: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let swap_bytes = swap.to_bytes();
        let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;

        invoke_signed(&ix, &[mint, destination, authority, token_program], signers)
    }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer<'a>(
        swap: &Pubkey,
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let swap_bytes = swap.to_bytes();
        let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;
        invoke_signed(
            &ix,
            &[source, destination, authority, token_program],
            signers,
        )
    }

    /// check if pda address is correct
    pub fn check_pda(program_id:&Pubkey, key: &Pubkey, tag: &str)->Result<(), ProgramError>{
        let seeds = [
            tag.as_bytes(),
            program_id.as_ref(),
        ];

        let (pda_key, _bump) = Pubkey::find_program_address(&seeds, program_id);
        if pda_key != *key {
            return Err(SwapError::InvalidPdaAddress.into());
        } 
        else {
            Ok(())
        }
    }

    /// create or allocate storage for new account
    pub fn create_or_allocate_account_raw<'a>(
        program_id: Pubkey,
        new_account_info: &AccountInfo<'a>,
        rent_sysvar_info: &AccountInfo<'a>,
        system_program_info: &AccountInfo<'a>,
        payer_info: &AccountInfo<'a>,
        size: usize,
        signer_seeds: &[&[u8]],
    ) -> Result<(), ProgramError> {
        let rent = &Rent::from_account_info(rent_sysvar_info)?;
        let required_lamports = rent
            .minimum_balance(size)
            .max(1)
            .saturating_sub(new_account_info.lamports());
    
        if required_lamports > 0 {
            msg!("Transfer {} lamports to the new account", required_lamports);
            invoke(
                &system_instruction::transfer(&payer_info.key, new_account_info.key, required_lamports),
                &[
                    payer_info.clone(),
                    new_account_info.clone(),
                    system_program_info.clone(),
                ],
            )?;
        }
    
        msg!("Allocate space for the account");
        invoke_signed(
            &system_instruction::allocate(new_account_info.key, size.try_into().map_err(|_| SwapError::InvalidAllocateSpaceForAccount)?),
            &[new_account_info.clone(), system_program_info.clone()],
            &[&signer_seeds],
        )?;
    
        msg!("Assign the account to the owning program");
        invoke_signed(
            &system_instruction::assign(new_account_info.key, &program_id),
            &[new_account_info.clone(), system_program_info.clone()],
            &[&signer_seeds],
        )?;
        msg!("Completed assignation!");
    
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn check_accounts(
        token_swap: &dyn SwapState,
        program_id: &Pubkey,
        swap_account_info: &AccountInfo,
        authority_info: &AccountInfo,
        token_a_info: &AccountInfo,
        token_b_info: &AccountInfo,
        pool_mint_info: &AccountInfo,
        token_program_info: &AccountInfo,
        user_token_a_info: Option<&AccountInfo>,
        user_token_b_info: Option<&AccountInfo>,
    ) -> ProgramResult {
        if swap_account_info.owner != program_id {
            return Err(ProgramError::IncorrectProgramId);
        }
        if *authority_info.key
            != Self::authority_id(program_id, swap_account_info.key, token_swap.nonce())?
        {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        if *token_a_info.key != *token_swap.token_a_account() {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if *token_b_info.key != *token_swap.token_b_account() {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if *pool_mint_info.key != *token_swap.pool_mint() {
            return Err(SwapError::IncorrectPoolMint.into());
        }
        if *token_program_info.key != *token_swap.token_program_id() {
            return Err(SwapError::IncorrectTokenProgramId.into());
        }
        if let Some(user_token_a_info) = user_token_a_info {
            if token_a_info.key == user_token_a_info.key {
                return Err(SwapError::InvalidInput.into());
            }
        }
        if let Some(user_token_b_info) = user_token_b_info {
            if token_b_info.key == user_token_b_info.key {
                return Err(SwapError::InvalidInput.into());
            }
        }
        Ok(())
    }
    
    /// processor for Global State
    pub fn process_set_global_state(
        program_id: &Pubkey,
        owner: &Pubkey,
        fee_owner: &Pubkey,
        initial_supply: u64,
        fees: Fees,
        swap_curve: SwapCurve,
        accounts: &[AccountInfo],
    ) -> ProgramResult {

        //load account info
        let account_info_iter = &mut accounts.iter();
        let global_state_info = next_account_info(account_info_iter)?;

        let current_owner_info = next_account_info(account_info_iter)?;

        let system_info = next_account_info(account_info_iter)?;
        let rent_info = next_account_info(account_info_iter)?;

        Self::check_pda(program_id, global_state_info.key, SWAP_TAG)?;
        
        if !current_owner_info.is_signer{
            return Err(SwapError::InvalidSigner.into());
        }

        if *system_info.key != Pubkey::from_str(SYSTEM_PROGRAM_ID).map_err(|_| SwapError::InvalidSystemProgramId)?{
            return Err(SwapError::InvalidSystemProgramId.into());
        }

        if *rent_info.key != Pubkey::from_str(RENT_SYSVAR_ID).map_err(|_| SwapError::InvalidRentSysvarId)?{
            return Err(SwapError::InvalidRentSysvarId.into());
        }

        let seeds = [
            SWAP_TAG.as_bytes(),
            program_id.as_ref(),
        ];

        let (_pda_key, bump) = Pubkey::find_program_address(&seeds, program_id);
        
        if global_state_info.data_is_empty(){
            let size = GlobalState::get_packed_len();

            Self::create_or_allocate_account_raw(
                *program_id,
                global_state_info,
                rent_info,
                system_info,
                current_owner_info,
                size,
                &[
                    SWAP_TAG.as_bytes(),
                    program_id.as_ref(),
                    &[bump],
                ],
            )?;
        }

        let mut global_state = GlobalState::unpack_from_slice(&global_state_info.data.borrow())?;

        if global_state.is_initialized == false
        {
            global_state.owner = Pubkey::from_str(INITIAL_PROGRAM_OWNER).map_err(|_| SwapError::InvalidProgramOwner)?;
            global_state.is_initialized = true;
            global_state.fees = Fees {
                trade_fee_numerator: INITIAL_FEES.trade_fee_numerator,
                trade_fee_denominator: INITIAL_FEES.trade_fee_denominator,
            };
            global_state.fee_owner = Pubkey::from_str(INITIAL_FEE_OWNER_KEY).map_err(|_| SwapError::IncorrectFeeAccount)?;
            global_state.initial_supply = INITIAL_SWAP_POOL_AMOUNT;
            global_state.swap_curve = SwapCurve {
                    curve_type: CurveType::ConstantProduct,
                    calculator: Box::new(
                        ConstantProductCurve{}
                    )
                };
        }
        
        if global_state.owner != *current_owner_info.key
        {
            return Err(SwapError::InvalidProgramOwner.into());
        }

        SWAP_CONSTRAINTS.validate_curve(&swap_curve)?;
        SWAP_CONSTRAINTS.validate_fees(&fees)?;

        fees.validate()?;
        swap_curve.calculator.validate()?;

        //Save the program state
        let obj = GlobalState{
            is_initialized:true,
            initial_supply: initial_supply,
            owner: *owner,
            fee_owner: *fee_owner,
            fees,
            swap_curve,
        };
        obj.pack_into_slice(&mut &mut global_state_info.data.borrow_mut()[..]);
        Ok(())
    }

    /// Processes an [Initialize](enum.Instruction.html).
    pub fn process_initialize(
        program_id: &Pubkey,
        nonce: u8,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let gloabal_state_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let fee_account_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_program_id = *token_program_info.key;
        if SwapVersion::is_initialized(&swap_info.data.borrow()) {
            return Err(SwapError::AlreadyInUse.into());
        }
        
        if *authority_info.key != Self::authority_id(program_id, swap_info.key, nonce)? {
            return Err(SwapError::InvalidProgramAddress.into());
        }

        Self::check_pda(program_id, gloabal_state_info.key, SWAP_TAG)?;
        
        let state = GlobalState::unpack_from_slice(&gloabal_state_info.data.borrow())?;
        if state.is_initialized() == false
        {
            return Err(SwapError::NotInitializedState.into());
        }

        let token_a = Self::unpack_token_account(token_a_info, &token_program_id)?;
        let token_b = Self::unpack_token_account(token_b_info, &token_program_id)?;
        let fee_account = Self::unpack_token_account(fee_account_info, &token_program_id)?;
        let destination = Self::unpack_token_account(destination_info, &token_program_id)?;
        let pool_mint = Self::unpack_mint(pool_mint_info, &token_program_id)?;
        if *authority_info.key != token_a.owner {
            return Err(SwapError::InvalidOwner.into());
        }
        if *authority_info.key != token_b.owner {
            return Err(SwapError::InvalidOwner.into());
        }
        if *authority_info.key == destination.owner {
            return Err(SwapError::InvalidOutputOwner.into());
        }
        if *authority_info.key == fee_account.owner {
            return Err(SwapError::InvalidOutputOwner.into());
        }
        if COption::Some(*authority_info.key) != pool_mint.mint_authority {
            return Err(SwapError::InvalidOwner.into());
        }

        if token_a.mint == token_b.mint {
            return Err(SwapError::RepeatedMint.into());
        }
        
        state.swap_curve()
            .calculator
            .validate_supply(token_a.amount, token_b.amount)?;

        if token_a.delegate.is_some() {
            return Err(SwapError::InvalidDelegate.into());
        }
        if token_b.delegate.is_some() {
            return Err(SwapError::InvalidDelegate.into());
        }
        if token_a.close_authority.is_some() {
            return Err(SwapError::InvalidCloseAuthority.into());
        }
        if token_b.close_authority.is_some() {
            return Err(SwapError::InvalidCloseAuthority.into());
        }

        if token_a.is_frozen(){
            return Err(SwapError::InvalidFreezeAuthority.into());
        }
        if token_b.is_frozen(){
            return Err(SwapError::InvalidFreezeAuthority.into());
        }

        if pool_mint.supply != 0 {
            return Err(SwapError::InvalidSupply.into());
        }
        if pool_mint.freeze_authority.is_some() {
            return Err(SwapError::InvalidFreezeAuthority.into());
        }
        // fixed
        if pool_mint.decimals != 8 {
            return Err(SwapError::MismatchDecimalValidation.into());
        }
        if *pool_mint_info.key != fee_account.mint {
            return Err(SwapError::IncorrectPoolMint.into());
        }

        let initial_amount = state.initial_supply();

        Self::token_mint_to(
            swap_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            destination_info.clone(),
            authority_info.clone(),
            nonce,
            initial_amount,
        )?;

        let obj = SwapVersion::SwapV1(SwapV1 {
            is_initialized: true,
            nonce,
            token_program_id,
            token_a: *token_a_info.key,
            token_b: *token_b_info.key,
            pool_mint: *pool_mint_info.key,
            token_a_mint: token_a.mint,
            token_b_mint: token_b.mint,
        });
        SwapVersion::pack(obj, &mut swap_info.data.borrow_mut())?;
        Ok(())
    }

    /// Processes an [Swap](enum.Instruction.html).
    pub fn process_swap(
        program_id: &Pubkey,
        amount_in: u64,
        minimum_amount_out: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        // get account info iterator
        let account_info_iter = &mut accounts.iter();
        // get swap info
        let swap_info = next_account_info(account_info_iter)?;
        // get authority info
        let authority_info = next_account_info(account_info_iter)?;
        // get user transfer autority info
        let user_transfer_authority_info = next_account_info(account_info_iter)?;

        let state_info = next_account_info(account_info_iter)?;
        // get source info
        let source_info = next_account_info(account_info_iter)?;
        // get swap source info
        let swap_source_info = next_account_info(account_info_iter)?;
        // get swap destination info
        let swap_destination_info = next_account_info(account_info_iter)?;
        // get destination info
        let destination_info = next_account_info(account_info_iter)?;
        // get pool mint info
        let pool_mint_info = next_account_info(account_info_iter)?;
        // get pool fee account info
        let pool_fee_account_info = next_account_info(account_info_iter)?;
        // get token program info
        let token_program_info = next_account_info(account_info_iter)?;
        // if swap owner is not program_id, then return incorrect program id error
        if swap_info.owner != program_id {
            return Err(ProgramError::IncorrectProgramId);
        }

        Self::check_pda(program_id, state_info.key, SWAP_TAG)?;
        
        let state = GlobalState::unpack_from_slice(&state_info.data.borrow())?;
        if state.is_initialized() == false
        {
            return Err(SwapError::NotInitializedState.into());
        }

        // get token_swap by swap_info.data
        let token_swap = SwapVersion::unpack(&swap_info.data.borrow())?;
        // if autority_info.key is not authority id then return invalid program address error
        if *authority_info.key != Self::authority_id(program_id, swap_info.key, token_swap.nonce())?
        {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        // if swap_source_info.key is token a account of token_swap or 
        // swap source info.key is token b account of token_swap then return incorrect swap account er
        if !(*swap_source_info.key == *token_swap.token_a_account()
            || *swap_source_info.key == *token_swap.token_b_account())
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        //if swap_destination_info.key is token a account of token_swap or 
        //swap_destination_info.key is token b account of token_swap then return incorrect swap account er
        if !(*swap_destination_info.key == *token_swap.token_a_account()
            || *swap_destination_info.key == *token_swap.token_b_account())
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        // if swap source info.key is swap destination key then return invalid input error
        if *swap_source_info.key == *swap_destination_info.key {
            return Err(SwapError::InvalidInput.into());
        }
        // if swap source info key is source info key then return invalid input
        if swap_source_info.key == source_info.key {
            return Err(SwapError::InvalidInput.into());
        }
        // if swap destination info key is destination info key then return invalid input key
        if swap_destination_info.key == destination_info.key {
            return Err(SwapError::InvalidInput.into());
        }
        // if pool mint info key is not token swap pool mint
        if *pool_mint_info.key != *token_swap.pool_mint() {
            return Err(SwapError::IncorrectPoolMint.into());
        }
        if *pool_fee_account_info.key != *state.fee_owner() {
            return Err(SwapError::IncorrectFeeAccount.into());
        }
        if *token_program_info.key != *token_swap.token_program_id() {
            return Err(SwapError::IncorrectTokenProgramId.into());
        }
        
        let source_account =
            Self::unpack_token_account(swap_source_info, token_swap.token_program_id())?;
        let dest_account =
            Self::unpack_token_account(swap_destination_info, token_swap.token_program_id())?;
        // let pool_mint = Self::unpack_mint(pool_mint_info, token_swap.token_program_id())?;

        let trade_direction = if *swap_source_info.key == *token_swap.token_a_account() {
            TradeDirection::AtoB
        } else {
            TradeDirection::BtoA
        };
        let result = state
            .swap_curve()
            .swap(
                to_u128(amount_in)?,
                to_u128(source_account.amount)?,
                to_u128(dest_account.amount)?,
                trade_direction,
                state.fees(),
            )
            .ok_or(SwapError::ZeroTradingTokens)?;
        if result.dest_amount < to_u128(minimum_amount_out)? {
            return Err(SwapError::ExceededSlippage.into());
        }

        // let (swap_token_a_amount, swap_token_b_amount) = match trade_direction {
        //     TradeDirection::AtoB => (
        //         result.new_swap_amount,
        //         result.dest_amount,
        //     ),
        //     TradeDirection::BtoA => (
        //         result.dest_amount,
        //         result.new_swap_amount,
        //     ),
        // };

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_info.clone(),
            swap_source_info.clone(),
            user_transfer_authority_info.clone(),
            token_swap.nonce(),
            to_u64(result.dest_amount)?,
        )?;

        // let mut pool_token_amount = token_swap
        //     .swap_curve()
        //     .withdraw_single_token_type_exact_out(
        //         result.owner_fee,
        //         swap_token_a_amount,
        //         swap_token_b_amount,
        //         to_u128(pool_mint.supply)?,
        //         trade_direction,
        //         token_swap.fees(),
        //     )
        //     .ok_or(SwapError::FeeCalculationFailure)?;

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            swap_destination_info.clone(),
            destination_info.clone(),
            authority_info.clone(),
            token_swap.nonce(),
            to_u64(result.dest_amount)?,
        )?;

        Ok(())
    }
    fn calculate_stable_lp_amount(
        total_supply:u128,
        token_a_amount: u128,
        token_b_amount: u128,
        input_a_amount: u128,
        input_b_amount: u128,
    )->Option<u128>{
        let total_amount = token_a_amount.checked_add(token_b_amount)?;
        let input_amount = input_a_amount.checked_add(input_b_amount)?;
        let pool_amount = input_amount.checked_mul(total_supply)?.checked_div(total_amount)?;
        
        Some(pool_amount)
        // u128::try_from(pool_amount).ok()
    }
    /// Processes an [DepositAllTokenTypes](enum.Instruction.html).
    pub fn process_deposit_all_token_types(
        program_id: &Pubkey,
        //pool_token_amount: u64,
        maximum_token_a_amount: u64,
        maximum_token_b_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let state_info = next_account_info(account_info_iter)?;
        let user_transfer_authority_info = next_account_info(account_info_iter)?;
        let source_a_info = next_account_info(account_info_iter)?;
        let source_b_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let dest_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_swap = SwapVersion::unpack(&swap_info.data.borrow())?;

        Self::check_pda(program_id, state_info.key, SWAP_TAG)?;
        let state = GlobalState::unpack_from_slice(&state_info.data.borrow())?;
        if state.is_initialized() == false
        {
            return Err(SwapError::NotInitializedState.into());
        }

        let calculator = &state.swap_curve().calculator;
        if !calculator.allows_deposits() {
            return Err(SwapError::UnsupportedCurveOperation.into());
        }
        Self::check_accounts(
            token_swap.as_ref(),
            program_id,
            swap_info,
            authority_info,
            token_a_info,
            token_b_info,
            pool_mint_info,
            token_program_info,
            Some(source_a_info),
            Some(source_b_info),
        )?;
        let token_a = Self::unpack_token_account(token_a_info, token_swap.token_program_id())?;
        let token_b = Self::unpack_token_account(token_b_info, token_swap.token_program_id())?;
        let pool_mint = Self::unpack_mint(pool_mint_info, token_swap.token_program_id())?;
                
        let pool_amount = Self::calculate_stable_lp_amount(
            to_u128(pool_mint.supply)?,
            to_u128(token_a.amount)?,
            to_u128(token_b.amount)?,
            to_u128(maximum_token_a_amount)?,
            to_u128(maximum_token_b_amount)?
        ).ok_or(SwapError::ZeroTradingTokens)?;

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_a_info.clone(),
            token_a_info.clone(),
            user_transfer_authority_info.clone(),
            token_swap.nonce(),
            maximum_token_a_amount,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_b_info.clone(),
            token_b_info.clone(),
            user_transfer_authority_info.clone(),
            token_swap.nonce(),
            maximum_token_b_amount,
        )?;
        Self::token_mint_to(
            swap_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            dest_info.clone(),
            authority_info.clone(),
            token_swap.nonce(),
            to_u64(pool_amount)?,
        )?;

        Ok(())
    }

    /// Processes an [WithdrawAllTokenTypes](enum.Instruction.html).
    pub fn process_withdraw_all_token_types(
        program_id: &Pubkey,
        pool_token_amount: u64,
        minimum_token_a_amount: u64,
        minimum_token_b_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let state_info = next_account_info(account_info_iter)?;
        let user_transfer_authority_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let dest_token_a_info = next_account_info(account_info_iter)?;
        let dest_token_b_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_swap = SwapVersion::unpack(&swap_info.data.borrow())?;

        Self::check_pda(program_id, state_info.key, SWAP_TAG)?;
        let state = GlobalState::unpack_from_slice(&state_info.data.borrow())?;
        if state.is_initialized() == false
        {
            return Err(SwapError::NotInitializedState.into());
        }

        Self::check_accounts(
            token_swap.as_ref(),
            program_id,
            swap_info,
            authority_info,
            token_a_info,
            token_b_info,
            pool_mint_info,
            token_program_info,
            Some(dest_token_a_info),
            Some(dest_token_b_info),
        )?;

        let token_a = Self::unpack_token_account(token_a_info, token_swap.token_program_id())?;
        let token_b = Self::unpack_token_account(token_b_info, token_swap.token_program_id())?;
        let pool_mint = Self::unpack_mint(pool_mint_info, token_swap.token_program_id())?;

        let calculator = &state.swap_curve().calculator;

        let mut pool_token_amount = to_u128(pool_token_amount)?;

        //Check the minimum lp token amount
        let max_pool_token_amount = to_u128(pool_mint.supply)?.checked_sub(MIN_LP_SUPPLY).ok_or(SwapError::CalculationFailure)?;
        pool_token_amount = std::cmp::min(pool_token_amount, max_pool_token_amount);

        let results = calculator
            .pool_tokens_to_trading_tokens(
                pool_token_amount,
                to_u128(pool_mint.supply)?,
                to_u128(token_a.amount)?,
                to_u128(token_b.amount)?,
                RoundDirection::Floor,
            )
            .ok_or(SwapError::ZeroTradingTokens)?;
        let token_a_amount = to_u64(results.token_a_amount)?;
        let token_a_amount = std::cmp::min(token_a.amount, token_a_amount);
        if token_a_amount < minimum_token_a_amount {
            return Err(SwapError::ExceededSlippage.into());
        }
        if token_a_amount == 0 && token_a.amount != 0 {
            return Err(SwapError::ZeroTradingTokens.into());
        }
        let token_b_amount = to_u64(results.token_b_amount)?;
        let token_b_amount = std::cmp::min(token_b.amount, token_b_amount);
        if token_b_amount < minimum_token_b_amount {
            return Err(SwapError::ExceededSlippage.into());
        }
        if token_b_amount == 0 && token_b.amount != 0 {
            return Err(SwapError::ZeroTradingTokens.into());
        }

        Self::token_burn(
            swap_info.key,
            token_program_info.clone(),
            source_info.clone(),
            pool_mint_info.clone(),
            user_transfer_authority_info.clone(),
            token_swap.nonce(),
            to_u64(pool_token_amount)?,
        )?;

        if token_a_amount > 0 {
            Self::token_transfer(
                swap_info.key,
                token_program_info.clone(),
                token_a_info.clone(),
                dest_token_a_info.clone(),
                authority_info.clone(),
                token_swap.nonce(),
                token_a_amount,
            )?;
        }
        if token_b_amount > 0 {
            Self::token_transfer(
                swap_info.key,
                token_program_info.clone(),
                token_b_info.clone(),
                dest_token_b_info.clone(),
                authority_info.clone(),
                token_swap.nonce(),
                token_b_amount,
            )?;
        }
        Ok(())
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = SwapInstruction::unpack(input)?;
        match instruction {
            SwapInstruction::Initialize(Initialize {
                nonce,
            }) => {
                msg!("Instruction: Init");
                Self::process_initialize(
                    program_id,
                    nonce,
                    accounts,
                )
            }
            SwapInstruction::Swap(Swap {
                amount_in,
                minimum_amount_out,
            }) => {
                msg!("Instruction: Swap");
                Self::process_swap(program_id, amount_in, minimum_amount_out, accounts)
            }
            SwapInstruction::DepositAllTokenTypes(DepositAllTokenTypes {
                //pool_token_amount,
                maximum_token_a_amount,
                maximum_token_b_amount,
            }) => {
                msg!("Instruction: DepositAllTokenTypes");
                Self::process_deposit_all_token_types(
                    program_id,
                    //pool_token_amount,
                    maximum_token_a_amount,
                    maximum_token_b_amount,
                    accounts,
                )
            }
            SwapInstruction::WithdrawAllTokenTypes(WithdrawAllTokenTypes {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            }) => {
                msg!("Instruction: WithdrawAllTokenTypes");
                Self::process_withdraw_all_token_types(
                    program_id,
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                    accounts,
                )
            }
            SwapInstruction::SetGlobalStateInstruction(SetGlobalState {
                owner,
                fee_owner,
                initial_supply,
                fees,
                swap_curve,
            }) => {
                msg!("Instruction: SetGlobalStateInstruction");
                Self::process_set_global_state(
                    program_id,
                    &owner,
                    &fee_owner,
                    initial_supply,
                    fees,
                    swap_curve,
                    accounts,
                )
            }
        }
    }
}

impl PrintProgramError for SwapError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            SwapError::AlreadyInUse => msg!("Error: Swap account already in use"),
            SwapError::InvalidProgramAddress => {
                msg!("Error: Invalid program address generated from nonce and key")
            }
            SwapError::InvalidOwner => {
                msg!("Error: The input account owner is not the program address")
            }
            SwapError::InvalidOutputOwner => {
                msg!("Error: Output pool account owner cannot be the program address")
            }
            SwapError::ExpectedMint => msg!("Error: Deserialized account is not an SPL Token mint"),
            SwapError::ExpectedAccount => {
                msg!("Error: Deserialized account is not an SPL Token account")
            }
            SwapError::EmptySupply => msg!("Error: Input token account empty"),
            SwapError::InvalidSupply => msg!("Error: Pool token mint has a non-zero supply"),
            SwapError::RepeatedMint => msg!("Error: Swap input token accounts have the same mint"),
            SwapError::InvalidDelegate => msg!("Error: Token account has a delegate"),
            SwapError::InvalidInput => msg!("Error: InvalidInput"),
            SwapError::IncorrectSwapAccount => {
                msg!("Error: Address of the provided swap token account is incorrect")
            }
            SwapError::IncorrectPoolMint => {
                msg!("Error: Address of the provided pool token mint is incorrect")
            }
            SwapError::InvalidOutput => msg!("Error: InvalidOutput"),
            SwapError::CalculationFailure => msg!("Error: CalculationFailure"),
            SwapError::InvalidInstruction => msg!("Error: InvalidInstruction"),
            SwapError::ExceededSlippage => {
                msg!("Error: Swap instruction exceeds desired slippage limit")
            }
            SwapError::InvalidCloseAuthority => msg!("Error: Token account has a close authority"),
            SwapError::InvalidFreezeAuthority => {
                msg!("Error: Pool token mint has a freeze authority")
            }
            SwapError::IncorrectFeeAccount => msg!("Error: Pool fee token account incorrect"),
            SwapError::ZeroTradingTokens => {
                msg!("Error: Given pool token amount results in zero trading tokens")
            }
            SwapError::FeeCalculationFailure => msg!(
                "Error: The fee calculation failed due to overflow, underflow, or unexpected 0"
            ),
            SwapError::ConversionFailure => msg!("Error: Conversion to or from u64 failed."),
            SwapError::InvalidFee => {
                msg!("Error: The provided fee does not match the program owner's constraints")
            }
            SwapError::IncorrectTokenProgramId => {
                msg!("Error: The provided token program does not match the token program expected by the swap")
            }
            SwapError::UnsupportedCurveType => {
                msg!("Error: The provided curve type is not supported by the program owner")
            }
            SwapError::InvalidCurve => {
                msg!("Error: The provided curve parameters are invalid")
            }
            SwapError::UnsupportedCurveOperation => {
                msg!("Error: The operation cannot be performed on the given curve")
            }
            SwapError::MismatchDecimalValidation => {
                msg!("The decimal validation error.(Decimal must be 8)")
            }

            SwapError::InvalidPdaAddress => {
                msg!("invalid program derived address")
            }
            SwapError::InvalidAllocateSpaceForAccount => {
                msg!("Can't allocate space for the account")
            }
            SwapError::InvalidSigner => {
                msg!("owner should be the signer")
            }
            SwapError::InvalidSystemProgramId => {
                msg!("Invalid SystemProgram Id")
            }
            SwapError::InvalidRentSysvarId => {
                msg!("Invalid Rent Sysvar Id")
            }
            SwapError::InvalidProgramOwner => {
                msg!("Invalid owner of the contract")
            }
            SwapError::NotInitializedState => {
                msg!("Program State should be initialized before creating pool")
            }
        }
    }
}

fn to_u128(val: u64) -> Result<u128, SwapError> {
    val.try_into().map_err(|_| SwapError::ConversionFailure)
}

fn to_u64(val: u128) -> Result<u64, SwapError> {
    val.try_into().map_err(|_| SwapError::ConversionFailure)
}