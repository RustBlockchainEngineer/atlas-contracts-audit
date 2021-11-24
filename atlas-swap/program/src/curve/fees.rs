//! All fee information, to be used for validation currently

use crate::error::SwapError;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};
use crate::{
    curve::{
        base::{SwapCurve, CurveType},
    },
};

use std::convert::TryFrom;

/// Encapsulates all fee information and calculations for swap operations
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fees {
    /// fee numerator to reinjected to the pool
    pub constant_product_return_fee_numerator: u64,
    
    /// fee numerator to reinjected to the owner account
    pub constant_product_fixed_fee_numerator: u64,

    /// fee numerator to reinjected to the pool
    pub stable_return_fee_numerator: u64,
    
    /// fee numerator to reinjected to the owner account
    pub stable_fixed_fee_numerator: u64,

    /// fee dominator 
    pub fee_denominator: u64
}

/// Helper function for calculating swap fee
pub fn calculate_fee(
    token_amount: u128,
    fee_numerator: u128,
    fee_denominator: u128,
) -> Option<u128> {
    if fee_numerator == 0 || token_amount == 0 {
        Some(0)
    } else {
        let fee = token_amount
            .checked_mul(fee_numerator)?
            .checked_div(fee_denominator)?;
        if fee == 0 {
            Some(1) // minimum fee of one token
        } else {
            Some(fee)
        }
    }
}

// fn validate_fraction(numerator: u64, denominator: u64) -> Result<(), SwapError> {
//     if denominator == 0 && numerator == 0 {
//         Ok(())
//     } else if numerator >= denominator {
//         Err(SwapError::InvalidFee)
//     } else {
//         Ok(())
//     }
// }

impl Fees {
    /// Calculate the withdraw fee in pool tokens
    pub fn return_fee(&self, trading_tokens: u128,swap_curve: &SwapCurve) -> Option<u128> {
        let mut return_fee_numerator = 0;
        match swap_curve.curve_type {
            CurveType::ConstantProduct => {
                return_fee_numerator = self.constant_product_return_fee_numerator;
            }
            CurveType::Stable => {
                return_fee_numerator = self.stable_return_fee_numerator;
            }
            _ => {
                return_fee_numerator = self.constant_product_return_fee_numerator;
            }
        }
        calculate_fee(
            trading_tokens,
            u128::try_from(return_fee_numerator).ok()?,
            u128::try_from(self.fee_denominator).ok()?,
        )
    }

    /// Calculate the trading fee in trading tokens
    pub fn fixed_fee(&self, trading_tokens: u128,swap_curve: &SwapCurve) -> Option<u128> {
        let mut fixed_fee_numerator = 0;
        match swap_curve.curve_type {
            CurveType::ConstantProduct => {
                fixed_fee_numerator = self.constant_product_fixed_fee_numerator;
            }
            CurveType::Stable => {
                fixed_fee_numerator = self.stable_fixed_fee_numerator;
            }
            _ => {
                fixed_fee_numerator = self.constant_product_fixed_fee_numerator;
            }
        }
        calculate_fee(
            trading_tokens,
            u128::try_from(fixed_fee_numerator).ok()?,
            u128::try_from(self.fee_denominator).ok()?,
        )
    }
    
    /// Validate that the fees are reasonable
    pub fn validate(&self) -> Result<(), SwapError> {

        if self.fee_denominator == 0 && 
            self.constant_product_fixed_fee_numerator == 0  && 
            self.stable_fixed_fee_numerator == 0  && 
            self.constant_product_return_fee_numerator == 0  && 
            self.stable_return_fee_numerator == 0
        {
            Ok(())
        } else if   self.constant_product_fixed_fee_numerator >= self.fee_denominator ||  
                    self.stable_fixed_fee_numerator >= self.fee_denominator || 
                    self.constant_product_return_fee_numerator >= self.fee_denominator || 
                    self.stable_return_fee_numerator >= self.fee_denominator || 
                    self.constant_product_fixed_fee_numerator >= self.fee_denominator - self.constant_product_return_fee_numerator ||
                    self.stable_fixed_fee_numerator >= self.fee_denominator - self.stable_return_fee_numerator
        {
            Err(SwapError::InvalidFee)
        } else {
            Ok(())
        }
    }
}

/// IsInitialized is required to use `Pack::pack` and `Pack::unpack`
impl IsInitialized for Fees {
    fn is_initialized(&self) -> bool {
        true
    }
}
impl Sealed for Fees {}
impl Pack for Fees {
    const LEN: usize = 40;
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 40];
        let (
            constant_product_return_fee_numerator,
            constant_product_fixed_fee_numerator,
            stable_return_fee_numerator,
            stable_fixed_fee_numerator,
            fee_denominator,
        ) = mut_array_refs![output, 8, 8, 8, 8, 8];
        *constant_product_return_fee_numerator = self.constant_product_return_fee_numerator.to_le_bytes();
        *constant_product_fixed_fee_numerator = self.constant_product_fixed_fee_numerator.to_le_bytes();
        *stable_return_fee_numerator = self.stable_return_fee_numerator.to_le_bytes();
        *stable_fixed_fee_numerator = self.stable_fixed_fee_numerator.to_le_bytes();
        *fee_denominator = self.fee_denominator.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Fees, ProgramError> {
        if input.len() < Self::LEN{
            return Err(SwapError::InvalidInstruction.into());    
        }
        let input = array_ref![input, 0, 40];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            constant_product_return_fee_numerator,
            constant_product_fixed_fee_numerator,
            stable_return_fee_numerator,
            stable_fixed_fee_numerator,
            fee_denominator,
        ) = array_refs![input, 8, 8, 8, 8, 8];
        Ok(Self {
            constant_product_return_fee_numerator: u64::from_le_bytes(*constant_product_return_fee_numerator),
            constant_product_fixed_fee_numerator: u64::from_le_bytes(*constant_product_fixed_fee_numerator),
            stable_return_fee_numerator: u64::from_le_bytes(*stable_return_fee_numerator),
            stable_fixed_fee_numerator: u64::from_le_bytes(*stable_fixed_fee_numerator),
            fee_denominator: u64::from_le_bytes(*fee_denominator),
        })
    }
}