//! Various constraints as required for production environments

use crate::{
    curve::{
        base::{CurveType, SwapCurve},
        fees::Fees,
    },
    error::SwapError,
};

use solana_program::program_error::ProgramError;

const MINIMUM_FEES: &Fees = &Fees {
    constant_product_return_fee_numerator: 0,
    constant_product_fixed_fee_numerator: 0,
    stable_return_fee_numerator: 0,
    stable_fixed_fee_numerator: 0,
    fee_denominator: 10000,
};
const VALID_CURVE_TYPES: &[CurveType] = &[CurveType::Stable, CurveType::ConstantProduct];


/// Encodes fee constraints, used in multihost environments where the program
/// may be used by multiple frontends, to ensure that proper fees are being
/// assessed.
/// Since this struct needs to be created at compile-time, we only have access
/// to const functions and constructors. Since SwapCurve contains a Box, it
/// cannot be used, so we have to split the curves based on their types.
pub struct SwapConstraints<'a> {
    /// Valid curve types
    pub valid_curve_types: &'a [CurveType],
    /// Valid fees
    pub fees: &'a Fees,
}

impl<'a> SwapConstraints<'a> {
    /// Checks that the provided curve is valid for the given constraints
    pub fn validate_curve(&self, swap_curve: &SwapCurve) -> Result<(), ProgramError> {
        if self
            .valid_curve_types
            .iter()
            .any(|x| *x == swap_curve.curve_type)
        {
            Ok(())
        } else {
            Err(SwapError::UnsupportedCurveType.into())
        }
    }

    /// Checks that the provided curve is valid for the given constraints
    pub fn validate_fees(&self, fees: &Fees) -> Result<(), ProgramError> {
        // msg!("{}, {}, {}, {}",fees.constant_product_return_fee_numerator,fees.constant_product_fixed_fee_numerator, fees.stable_return_fee_numerator, fees.stable_fixed_fee_numerator);
        // msg!("{}, {}, {}, {}",self.fees.constant_product_return_fee_numerator,self.fees.constant_product_fixed_fee_numerator, self.fees.stable_return_fee_numerator, self.fees.stable_fixed_fee_numerator);
        if fees.constant_product_return_fee_numerator >= self.fees.constant_product_return_fee_numerator
            && fees.constant_product_fixed_fee_numerator >= self.fees.constant_product_fixed_fee_numerator
            && fees.stable_return_fee_numerator >= self.fees.stable_return_fee_numerator
            && fees.stable_fixed_fee_numerator >= self.fees.stable_fixed_fee_numerator
        {
            Ok(())
        } else {
            Err(SwapError::InvalidFee.into())
        }
    }
}

/// swap tag for seeds
pub const SWAP_TAG:&str = "atals-swap";

/// rent sysvar program id
pub const RENT_SYSVAR_ID:&str = "SysvarRent111111111111111111111111111111111";

/// system program id
pub const SYSTEM_PROGRAM_ID:&str = "11111111111111111111111111111111";

/// initial program owner address
pub const INITIAL_PROGRAM_OWNER: &str = "ArNRr3C8DATnfbGFTQyPe38cBgXwkfon1CHSxgesb1rG";

/// swap contraints
pub const SWAP_CONSTRAINTS:SwapConstraints = SwapConstraints {
    valid_curve_types: VALID_CURVE_TYPES,
    fees: MINIMUM_FEES,
};

/// minimum lp supply
pub const MIN_LP_SUPPLY:u128 = 100000;