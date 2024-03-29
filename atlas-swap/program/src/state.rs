//! State transition types

use crate::curve::{base::{SwapCurve}, fees::Fees};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use enum_dispatch::enum_dispatch;
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,

};
use crate::error::SwapError;

/// Trait representing access to program state across all versions
#[enum_dispatch]
pub trait SwapState {
    /// Is the swap initialized, with data written to it
    fn is_initialized(&self) -> bool;
    /// Bump seed used to generate the program address / authority
    fn nonce(&self) -> u8;
    /// Token program ID associated with the swap
    fn token_program_id(&self) -> &Pubkey;
    /// Address of token A liquidity account
    fn token_a_account(&self) -> &Pubkey;
    /// Address of token B liquidity account
    fn token_b_account(&self) -> &Pubkey;
    /// Address of pool token mint
    fn pool_mint(&self) -> &Pubkey;

    /// Address of token A mint
    fn token_a_mint(&self) -> &Pubkey;
    /// Address of token B mint
    fn token_b_mint(&self) -> &Pubkey;
    ///
    fn swap_curve(&self) -> &SwapCurve;
}


/// All versions of SwapState
#[enum_dispatch(SwapState)]
pub enum SwapVersion {
    /// Latest version, used for all new swaps
    SwapV1,
}

/// SwapVersion does not implement program_pack::Pack because there are size
/// checks on pack and unpack that would break backwards compatibility, so
/// special implementations are provided here
impl SwapVersion {
    /// Size of the latest version of the SwapState
    pub const LATEST_LEN: usize = 1 + SwapV1::LEN; // add one for the version enum

    /// Pack a swap into a byte array, based on its version
    pub fn pack(src: Self, dst: &mut [u8]) -> Result<(), ProgramError> {
        match src {
            Self::SwapV1(swap_info) => {
                dst[0] = 1;
                SwapV1::pack(swap_info, &mut dst[1..])
            }
        }
    }

    /// Unpack the swap account based on its version, returning the result as a
    /// SwapState trait object
    pub fn unpack(input: &[u8]) -> Result<Box<dyn SwapState>, ProgramError> {
        let (&version, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidAccountData)?;
        match version {
            1 => Ok(Box::new(SwapV1::unpack(rest)?)),
            _ => Err(ProgramError::UninitializedAccount),
        }
    }

    /// Special check to be done before any instruction processing, works for
    /// all versions
    pub fn is_initialized(input: &[u8]) -> bool {
        match Self::unpack(input) {
            Ok(swap) => swap.is_initialized(),
            Err(_) => false,
        }
    }
}

/// Program states.
#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct SwapV1 {
    /// Initialized state.
    pub is_initialized: bool,
    /// Nonce used in program address.
    /// The program address is created deterministically with the nonce,
    /// swap program id, and swap account pubkey.  This program address has
    /// authority over the swap's token A account, token B account, and pool
    /// token mint.
    pub nonce: u8,

    /// Program ID of the tokens being exchanged.
    pub token_program_id: Pubkey,

    /// Token A
    pub token_a: Pubkey,
    /// Token B
    pub token_b: Pubkey,

    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    pub pool_mint: Pubkey,

    /// Mint information for token A
    pub token_a_mint: Pubkey,
    /// Mint information for token B
    pub token_b_mint: Pubkey,

    ///Curve Type to swap
    pub swap_curve: SwapCurve,
    
}

impl SwapState for SwapV1 {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    fn nonce(&self) -> u8 {
        self.nonce
    }

    fn token_program_id(&self) -> &Pubkey {
        &self.token_program_id
    }

    fn token_a_account(&self) -> &Pubkey {
        &self.token_a
    }

    fn token_b_account(&self) -> &Pubkey {
        &self.token_b
    }

    fn pool_mint(&self) -> &Pubkey {
        &self.pool_mint
    }

    fn token_a_mint(&self) -> &Pubkey {
        &self.token_a_mint
    }

    fn token_b_mint(&self) -> &Pubkey {
        &self.token_b_mint
    }

    fn swap_curve(&self) -> &SwapCurve {
        &self.swap_curve
    }

}

impl Sealed for SwapV1 {}
impl IsInitialized for SwapV1 {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for SwapV1 {
    const LEN: usize = 227;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, SwapV1::LEN];
        let (
            is_initialized,
            nonce,
            token_program_id,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            swap_curve,
        ) = mut_array_refs![output, 1, 1, 32, 32, 32, 32, 32, 32, 33];
        is_initialized[0] = self.is_initialized as u8;
        nonce[0] = self.nonce;
        token_program_id.copy_from_slice(self.token_program_id.as_ref());
        token_a.copy_from_slice(self.token_a.as_ref());
        token_b.copy_from_slice(self.token_b.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
        token_a_mint.copy_from_slice(self.token_a_mint.as_ref());
        token_b_mint.copy_from_slice(self.token_b_mint.as_ref());
        self.swap_curve.pack_into_slice(&mut swap_curve[..]);
    }

    /// Unpacks a byte buffer into a [SwapV1](struct.SwapV1.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        if input.len() < Self::LEN {
            return Err(ProgramError::MaxSeedLengthExceeded);
        }
        let input = array_ref![input, 0, SwapV1::LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            nonce,
            token_program_id,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            swap_curve,
        ) = array_refs![input, 1, 1, 32, 32, 32, 32, 32, 32, 33];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            nonce: nonce[0],
            token_program_id: Pubkey::new_from_array(*token_program_id),
            token_a: Pubkey::new_from_array(*token_a),
            token_b: Pubkey::new_from_array(*token_b),
            pool_mint: Pubkey::new_from_array(*pool_mint),
            token_a_mint: Pubkey::new_from_array(*token_a_mint),
            token_b_mint: Pubkey::new_from_array(*token_b_mint),
            swap_curve: SwapCurve::unpack_from_slice(swap_curve)?,
        })
    }
}

///Program State
#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct GlobalState {
    /// Initialized state.
    pub is_initialized:bool,

    /// program owner address to update all
    pub owner: Pubkey,

    /// Fee owner address
    pub fee_owner: Pubkey,

    /// initial lp supply
    pub initial_supply: u64,

    /// lp token's decimals
    pub lp_decimals: u8,

    ///Fee ratio
    pub fees: Fees,
}
impl Sealed for GlobalState {}
impl Pack for GlobalState{
    /// Size of the Program State
    const LEN:usize = 114; // add one for the version enum

    /// Pack a swap into a byte array, based on its version
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, GlobalState::LEN];
        let (
            is_initialized,
            state_owner,
            fee_owner,
            initial_supply,
            lp_decimals,
            fees,
        ) = mut_array_refs![output, 1, 32, 32, 8, 1, 40];
        is_initialized[0] = self.is_initialized as u8;
        state_owner.copy_from_slice(self.owner.as_ref());
        fee_owner.copy_from_slice(self.fee_owner.as_ref());
        *initial_supply = self.initial_supply.to_le_bytes();
        lp_decimals[0] = self.lp_decimals as u8;
        self.fees.pack_into_slice(&mut fees[..]);
    }

    /// Unpacks a byte buffer into a [SwapV1](struct.SwapV1.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        if input.len() != GlobalState::LEN{
            return Err(SwapError::InvalidInstruction.into());    
        }
        let input = array_ref![input, 0, GlobalState::LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            state_owner,
            fee_owner,
            initial_supply,
            lp_decimals,
            fees,
        ) = array_refs![input, 1, 32, 32, 8, 1, 40];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            owner: Pubkey::new_from_array(*state_owner),
            fee_owner: Pubkey::new_from_array(*fee_owner),
            initial_supply:u64::from_le_bytes(*initial_supply),
            lp_decimals:lp_decimals[0],
            fees: Fees::unpack_from_slice(fees)?,
        })
    }
}

impl GlobalState{
    /// is program account initialized
    pub fn is_initialized(&self) -> bool {
        return self.is_initialized
    }
    /// state owner to change current program state
    pub fn owner(&self) -> &Pubkey {
        &self.owner
    }

    /// fee owner to recevie when swap
    pub fn fee_owner(&self) -> &Pubkey {
        &self.fee_owner
    }

    /// initial supply to create pool
    pub fn initial_supply(&self) -> u64 {
        self.initial_supply
    }

    /// lp decimals
    pub fn lp_decimals(&self) -> u8 {
        self.lp_decimals
    }
    
    /// fees redistributed
    pub fn fees(&self) -> &Fees {
        &self.fees
    }
}
