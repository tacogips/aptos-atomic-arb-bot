use crate::swap::amm::{CONST_ONE, CONST_THREE, CONST_TWO, CONST_ZERO, EIGHT_FIG_NUM, U256};
use anyhow::{anyhow, Result};
use num_integer::Integer;

macro_rules! scale_norm {
    ($x:expr,$scale:expr) => {{
        let x = U256::try_from($x)?;
        let d = U256::try_from($scale)?;
        x * *EIGHT_FIG_NUM / d
    }};
}

macro_rules! scale_denorm {
    ($x:expr,$y:expr) => {{
        let x = U256::try_from($x)?;
        let d = U256::try_from($y)?;
        x * d / (*EIGHT_FIG_NUM)
    }};
}

// k = x^3 * y + x*y^3
macro_rules! invariant {
    ($x:expr,$y:expr) => {
        $x * $y * (($x.pow(*CONST_TWO)) + ($y.pow(*CONST_TWO)))
    };
}

// f(x,y) = (x^3*y + x * y^3)
// df/dy =  (x^3   + 3 * x * y^2)
macro_rules! difference_y {
    ($x:expr, $y:expr) => {
        ($x.pow(*CONST_THREE)) + (*CONST_THREE * $x * $y.pow(*CONST_TWO))
    };
}

// x^3 * y + x * y^3 = k
pub fn get_liquid_stable_coin_out_with_fee(
    amount_in: u64,
    reserve_in: u64,
    reserve_out: u64,

    scale_in: u64,
    scale_out: u64,

    fee_rate: u64,
    fee_scale: u64,
) -> Result<u64> {
    if fee_scale == 0 {
        return Err(anyhow!(
            "fee_scale is 0 in get_liquid_stable_coin_out_with_fee"
        ));
    }
    let fee_multiplier = fee_scale - fee_rate; // e.g. 10000 - 30 => 9970
    let amount_in_val_after_fees_scaled = amount_in as u128 * fee_multiplier as u128;

    let (mut amount_in, remaider) = amount_in_val_after_fees_scaled.div_rem(&(fee_scale as u128));
    if remaider != 0 {
        amount_in += 1;
    }

    let amount_in = scale_norm!(amount_in, scale_in);
    let reserve_in = scale_norm!(reserve_in, scale_in);
    let reserve_out = scale_norm!(reserve_out, scale_out);

    let k: U256 = invariant!(reserve_in, reserve_out);

    let reserve_in_after = reserve_in + amount_in;
    let reserve_out_after =
        reserve_out - calc_liquid_stable_reserve_after(reserve_in_after, k, reserve_out)?;

    let out = scale_denorm!(reserve_out_after, scale_out);
    Ok(out.as_u64())
}

fn calc_liquid_stable_reserve_after(
    reserve_in_after: U256,
    k: U256,
    mut reserve_out: U256,
) -> Result<U256> {
    for _i in 0..=255 {
        let current_k = invariant!(reserve_in_after, reserve_out);
        let mut d_y = difference_y!(reserve_in_after, reserve_out);
        if current_k < k {
            d_y = (k - current_k) / d_y + *CONST_ONE; //round up
            reserve_out += d_y;
        } else if d_y != *CONST_ZERO {
            d_y = (current_k - k) / d_y;
            reserve_out -= d_y;
        }
        if d_y <= *CONST_ONE {
            return Ok(reserve_out);
        }
    }

    Ok(reserve_out)
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_differnece_1() {
        let x0 = U256::try_from(10000518365287u128).unwrap();
        let y = U256::try_from(2520572000001255u128).unwrap();

        let z = difference_y!(x0, y);
        let z = (z / *EIGHT_FIG_NUM).as_u128();
        assert_eq!(z, 1906093763356467088703995764640866982)
    }

    #[test]
    fn test_differnece_2() {
        let x0 = U256::try_from(5000000000u128).unwrap();
        let y = U256::try_from(10000000000000000u128).unwrap();

        let z = difference_y!(x0, y);
        let z = (z / *EIGHT_FIG_NUM).as_u128();
        assert_eq!(z, 15000000000001250000000000000000000)
    }

    #[test]
    fn test_differnece_3() {
        let x0 = U256::try_from(1).unwrap();
        let y = U256::try_from(2).unwrap();

        let z = difference_y!(x0, y);
        assert_eq!(z, U256::try_from(13).unwrap())
    }

    #[test]
    fn test_liquid_stable_k_1() {
        let x = U256::try_from(10000518365287u128).unwrap();
        let y = U256::try_from(2520572000001255u128).unwrap();

        let z = invariant!(x, y);
        let z = (z / U256::try_from(1000000000000000000000000u128).unwrap()).as_u128();
        assert_eq!(z, 160149899619106589403934712464197979)
    }

    #[test]
    fn test_get_liquid_stable_amount_out() {
        let actual = get_liquid_stable_coin_out_with_fee(
            2513058000,
            25582858050757,
            2558285805075712,
            1000000,
            100000000,
            0,
            10000,
        )
        .unwrap();

        assert_eq!(actual, 251305799999);
    }
}
