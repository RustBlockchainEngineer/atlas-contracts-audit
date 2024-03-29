//! Instruction types

#![allow(clippy::too_many_arguments)]

use crate::curve::{base::SwapCurve, fees::Fees};
use crate::error::SwapError;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    msg,
    system_program,
    sysvar
};
use std::convert::TryInto;
use std::mem::size_of;

#[cfg(feature = "fuzz")]
use arbitrary::Arbitrary;

/// Initialize instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct Initialize {
    /// swap curve info for pool, including CurveType and anything
    /// else that may be required
    pub swap_curve: SwapCurve,
}


/// Set Global State data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct SetGlobalState {

    /// program owner address to update all
    pub owner: Pubkey,

    /// Fee owner address
    pub fee_owner: Pubkey,

    /// initial lp supply
    pub initial_supply: u64,

    /// lp decimals
    pub lp_decimals: u8,

    ///Fee ratio
    pub fees: Fees,

}


/// Swap instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct Swap {
    /// SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    pub amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    pub minimum_amount_out: u64,
}

/// DepositAllTokenTypes instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositAllTokenTypes {
    /// Pool token amount to transfer. token_a and token_b amount are set by
    /// the current exchange rate and size of the pool
    pub pool_token_amount: u64,
    /// Maximum token A amount to deposit, prevents excessive slippage
    pub maximum_token_a_amount: u64,
    /// Maximum token B amount to deposit, prevents excessive slippage
    pub maximum_token_b_amount: u64,
}

/// WithdrawAllTokenTypes instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct WithdrawAllTokenTypes {
    /// Amount of pool tokens to burn. User receives an output of token a
    /// and b based on the percentage of the pool tokens that are returned.
    pub pool_token_amount: u64,
    /// Minimum amount of token A to receive, prevents excessive slippage
    pub minimum_token_a_amount: u64,
    /// Minimum amount of token B to receive, prevents excessive slippage
    pub minimum_token_b_amount: u64,
}

/// Deposit one token type, exact amount in instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositSingleTokenTypeExactAmountIn {
    /// Token amount to deposit
    pub source_token_amount: u64,
    /// Pool token amount to receive in exchange. The amount is set by
    /// the current exchange rate and size of the pool
    pub minimum_pool_token_amount: u64,
}

/// WithdrawAllTokenTypes instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct WithdrawSingleTokenTypeExactAmountOut {
    /// Amount of token A or B to receive
    pub destination_token_amount: u64,
    /// Maximum amount of pool tokens to burn. User receives an output of token A
    /// or B based on the percentage of the pool tokens that are returned.
    pub maximum_pool_token_amount: u64,
}

/// Instructions supported by the token swap program.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum SwapInstruction {
    ///   Initializes a new swap
    ///
    ///   0. `[writable, signer]` New Token-swap to create.
    ///   1. `[]` swap authority derived from `create_program_address(&[Token-swap account])`
    ///   2. `[]` token_a Account. Must be non zero, owned by swap authority.
    ///   3. `[]` token_b Account. Must be non zero, owned by swap authority.
    ///   4. `[writable]` Pool Token Mint. Must be empty, owned by swap authority.
    ///   5. `[]` Pool Token Account to deposit trading and withdraw fees.
    ///   Must be empty, not owned by swap authority
    ///   6. `[writable]` Pool Token Account to deposit the initial pool token
    ///   supply.  Must be empty, not owned by swap authority.
    ///   7. '[]` Token program id
    Initialize(Initialize),

    ///   Swap the tokens in the pool.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` swap authority
    ///   2. `[]` user transfer authority
    ///   3. `[writable]` token_(A|B) SOURCE Account, amount is transferable by user transfer authority,
    ///   4. `[writable]` token_(A|B) Base Account to swap INTO.  Must be the SOURCE token.
    ///   5. `[writable]` token_(A|B) Base Account to swap FROM.  Must be the DESTINATION token.
    ///   6. `[writable]` token_(A|B) DESTINATION Account assigned to USER as the owner.
    ///   7. `[writable]` Pool token mint, to generate trading fees
    ///   8. `[writable]` Fee account, to receive trading fees
    ///   9. '[]` Token program id
    ///   10 `[optional, writable]` Host fee account to receive additional trading fees
    Swap(Swap),

    ///   Deposit both types of tokens into the pool.  The output is a "pool"
    ///   token representing ownership in the pool. Inputs are converted to
    ///   the current ratio.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` swap authority
    ///   2. `[]` user transfer authority
    ///   3. `[writable]` token_a user transfer authority can transfer amount,
    ///   4. `[writable]` token_b user transfer authority can transfer amount,
    ///   5. `[writable]` token_a Base Account to deposit into.
    ///   6. `[writable]` token_b Base Account to deposit into.
    ///   7. `[writable]` Pool MINT account, swap authority is the owner.
    ///   8. `[writable]` Pool Account to deposit the generated tokens, user is the owner.
    ///   9. '[]` Token program id
    DepositAllTokenTypes(DepositAllTokenTypes),

    ///   Withdraw both types of tokens from the pool at the current ratio, given
    ///   pool tokens.  The pool tokens are burned in exchange for an equivalent
    ///   amount of token A and B.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` swap authority
    ///   2. `[]` user transfer authority
    ///   3. `[writable]` Pool mint account, swap authority is the owner
    ///   4. `[writable]` SOURCE Pool account, amount is transferable by user transfer authority.
    ///   5. `[writable]` token_a Swap Account to withdraw FROM.
    ///   6. `[writable]` token_b Swap Account to withdraw FROM.
    ///   7. `[writable]` token_a user Account to credit.
    ///   8. `[writable]` token_b user Account to credit.
    ///   9. `[writable]` Fee account, to receive withdrawal fees
    ///   10 '[]` Token program id
    WithdrawAllTokenTypes(WithdrawAllTokenTypes),

    ///   Set global program state
    ///
    ///   0. `[writable]` program state account
    ///   1. `[]` owner of  this contract
    ///   2. `[]` owner address to update.
    ///   3. `[]` fee owner address to update.
    ///   4. `[]` initial supply
    ///   5. `[]` fees
    ///   6. `[]` swap curve.
    SetGlobalStateInstruction(SetGlobalState),

}

impl SwapInstruction {
    /// Unpacks a byte buffer into a [SwapInstruction](enum.SwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        msg!("unpack instruction");
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        msg!("unpack instruction tag {}", tag);
        Ok(match tag {
            0 => {
                let swap_curve = SwapCurve::unpack_unchecked(rest)?;
                msg!("unpack instruction rest.len() {}", rest.len());
                // if rest.len() == 1 {
                    Self::Initialize(Initialize {
                        swap_curve,
                    })
                // } else {
                //     return Err(SwapError::InvalidInstruction.into());
                // }
            }
            1 => {
                let (amount_in, rest) = Self::unpack_u64(rest)?;
                let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                Self::Swap(Swap {
                    amount_in,
                    minimum_amount_out,
                })
            }
            2 => {
                let (pool_token_amount, rest) = Self::unpack_u64(rest)?;
                let (maximum_token_a_amount, rest) = Self::unpack_u64(rest)?;
                let (maximum_token_b_amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositAllTokenTypes(DepositAllTokenTypes {
                    pool_token_amount,
                    maximum_token_a_amount,
                    maximum_token_b_amount,
                })
            }
            3 => {
                let (pool_token_amount, rest) = Self::unpack_u64(rest)?;
                let (minimum_token_a_amount, rest) = Self::unpack_u64(rest)?;
                let (minimum_token_b_amount, _rest) = Self::unpack_u64(rest)?;
                Self::WithdrawAllTokenTypes(WithdrawAllTokenTypes {
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                })
            }
            4 => {// Upgrade Program State
                let (owner_vec, rest) = rest.split_at(32);
                let owner = Pubkey::new(owner_vec);
                let (fee_owner_vec, rest) = rest.split_at(32);
                let fee_owner = Pubkey::new(fee_owner_vec);

                let (initial_supply, rest) = Self::unpack_u64(rest)?;
                let (&lp_decimals, rest) = rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                if rest.len() >= Fees::LEN {
                    let (fees, _rest) = rest.split_at(Fees::LEN);
                    let fees = Fees::unpack_unchecked(fees)?;
                    Self::SetGlobalStateInstruction(SetGlobalState {
                        owner,
                        fee_owner,
                        initial_supply,
                        lp_decimals,
                        fees,
                    })
                } else {
                    return Err(SwapError::InvalidInstruction.into());
                }
            }
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (amount, rest) = input.split_at(8);
            let amount = amount
                .get(..8)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(SwapError::InvalidInstruction)?;
            Ok((amount, rest))
        } else {
            Err(SwapError::InvalidInstruction.into())
        }
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match &*self {
            Self::Initialize(Initialize {
                swap_curve
            }) => {
                buf.push(0);
                let mut swap_curve_slice = [0u8; SwapCurve::LEN];
                Pack::pack_into_slice(swap_curve, &mut swap_curve_slice[..]);
                buf.extend_from_slice(&swap_curve_slice);
            }
            Self::Swap(Swap {
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(1);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
            Self::DepositAllTokenTypes(DepositAllTokenTypes {
                pool_token_amount,
                maximum_token_a_amount,
                maximum_token_b_amount,
            }) => {
                buf.push(2);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&maximum_token_a_amount.to_le_bytes());
                buf.extend_from_slice(&maximum_token_b_amount.to_le_bytes());
            }
            Self::WithdrawAllTokenTypes(WithdrawAllTokenTypes {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            }) => {
                buf.push(3);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
            }
            Self::SetGlobalStateInstruction(SetGlobalState {
                owner,
                fee_owner,
                initial_supply,
                lp_decimals,
                fees,
            }) => {
                buf.push(4);
                buf.extend_from_slice(owner.as_ref());
                buf.extend_from_slice(fee_owner.as_ref());
                buf.extend_from_slice(&initial_supply.to_le_bytes());
                buf.extend_from_slice(&lp_decimals.to_le_bytes());
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    token_a_pubkey: &Pubkey,
    token_b_pubkey: &Pubkey,
    pool_pubkey: &Pubkey,
    fee_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
    swap_curve: SwapCurve,
) -> Result<Instruction, ProgramError> {
    let init_data = SwapInstruction::Initialize(Initialize {
        swap_curve
    });
    let data = init_data.pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*token_a_pubkey, false),
        AccountMeta::new_readonly(*token_b_pubkey, false),
        AccountMeta::new(*pool_pubkey, false),
        AccountMeta::new_readonly(*fee_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'deposit_all_token_types' instruction.
pub fn deposit_all_token_types(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    user_transfer_authority_pubkey: &Pubkey,
    deposit_token_a_pubkey: &Pubkey,
    deposit_token_b_pubkey: &Pubkey,
    swap_token_a_pubkey: &Pubkey,
    swap_token_b_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
    instruction: DepositAllTokenTypes,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::DepositAllTokenTypes(instruction).pack();

    let accounts = vec![
        AccountMeta::new_readonly(*swap_pubkey, false),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*user_transfer_authority_pubkey, true),
        AccountMeta::new(*deposit_token_a_pubkey, false),
        AccountMeta::new(*deposit_token_b_pubkey, false),
        AccountMeta::new(*swap_token_a_pubkey, false),
        AccountMeta::new(*swap_token_b_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'withdraw_all_token_types' instruction.
pub fn withdraw_all_token_types(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    user_transfer_authority_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    fee_account_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    swap_token_a_pubkey: &Pubkey,
    swap_token_b_pubkey: &Pubkey,
    destination_token_a_pubkey: &Pubkey,
    destination_token_b_pubkey: &Pubkey,
    instruction: WithdrawAllTokenTypes,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::WithdrawAllTokenTypes(instruction).pack();

    let accounts = vec![
        AccountMeta::new_readonly(*swap_pubkey, false),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*user_transfer_authority_pubkey, true),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*swap_token_a_pubkey, false),
        AccountMeta::new(*swap_token_b_pubkey, false),
        AccountMeta::new(*destination_token_a_pubkey, false),
        AccountMeta::new(*destination_token_b_pubkey, false),
        AccountMeta::new(*fee_account_pubkey, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'swap' instruction.
pub fn swap(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    user_transfer_authority_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    swap_source_pubkey: &Pubkey,
    swap_destination_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    pool_fee_pubkey: &Pubkey,
    host_fee_pubkey: Option<&Pubkey>,
    instruction: Swap,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Swap(instruction).pack();

    let mut accounts = vec![
        AccountMeta::new_readonly(*swap_pubkey, false),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*user_transfer_authority_pubkey, true),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*swap_source_pubkey, false),
        AccountMeta::new(*swap_destination_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*pool_fee_pubkey, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];
    if let Some(host_fee_pubkey) = host_fee_pubkey {
        accounts.push(AccountMeta::new(*host_fee_pubkey, false));
    }

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates an 'SetGlobalStateInstruction' instruction.
pub fn set_global_state(
    program_id: &Pubkey,
    state_account_pubkey: &Pubkey,
    current_owner_pubkey: &Pubkey,
    owner_pubkey: &Pubkey,
    fee_owner_pubkey: &Pubkey,
    initial_supply: u64,
    lp_decimals: u8,
    fees: Fees,
) -> Result<Instruction, ProgramError> {
    let init_data = SwapInstruction::SetGlobalStateInstruction(SetGlobalState {
        owner:*owner_pubkey,
        fee_owner:*fee_owner_pubkey,
        initial_supply,
        lp_decimals,
        fees,
    });
    let data = init_data.pack();

    let accounts = vec![
        AccountMeta::new(*state_account_pubkey, false),
        AccountMeta::new_readonly(*current_owner_pubkey, true),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}
