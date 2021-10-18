use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::Decimal;

/// return a * b
pub fn decimal_multiplication_in_256(a: Decimal, b: Decimal) -> Decimal {
    let a_u256: Decimal256 = a.into();
    let b_u256: Decimal256 = b.into();
    let c_u256: Decimal = (b_u256 * a_u256).into();
    c_u256
}

/// return a - b
pub fn decimal_subtraction_in_256(a: Decimal, b: Decimal) -> Decimal {
    let a_u256: Decimal256 = a.into();
    let b_u256: Decimal256 = b.into();
    let c_u256: Decimal = (a_u256 - b_u256).into();
    c_u256
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{Decimal, Uint128};

    #[test]
    fn test_decimal_multiplication() {
        let a = Uint128::from(100_u128);
        let b = Decimal::from_ratio(Uint128::from(1111111_u128), Uint128::from(10000000_u128));
        let multiplication =
            decimal_multiplication_in_256(Decimal::from_ratio(a, Uint128::from(1_u128)), b);
        assert_eq!(multiplication.to_string(), "11.11111");
    }

    #[test]
    fn test_decimal_subtraction() {
        let a = Decimal::from_ratio(Uint128::from(20_u128), Uint128::from(50_u128));
        let b = Decimal::from_ratio(Uint128::from(10_u128), Uint128::from(50_u128));
        let res = decimal_subtraction_in_256(a, b);
        assert_eq!(res.to_string(), "0.2");
    }

    #[test]
    fn test_decimal_multiplication_in_256() {
        let a = Uint128::from(100_u128);
        let b = Decimal::from_ratio(Uint128::from(1111111_u128), Uint128::from(10000000_u128));
        let multiplication =
            decimal_multiplication_in_256(Decimal::from_ratio(a, Uint128::from(1_u128)), b);
        assert_eq!(multiplication.to_string(), "11.11111");
    }

    #[test]
    fn test_decimal_subtraction_in_256() {
        let a = Decimal::from_ratio(Uint128::from(20_u128), Uint128::from(50_u128));
        let b = Decimal::from_ratio(Uint128::from(10_u128), Uint128::from(50_u128));
        let res = decimal_subtraction_in_256(a, b);
        assert_eq!(res.to_string(), "0.2");
    }
}
