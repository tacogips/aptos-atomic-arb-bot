use crate::swap::amm::{CONST_TWO, CONST_ZERO, U256};
use anyhow::{anyhow, Result};
use static_init::dynamic;

const MAX_LOOP_LIMIT: u64 = 100;

#[dynamic]
pub static CONVERGE_THRESHOLD: U256 = U256::try_from(3_u64).unwrap();

pub fn calc_out_given_in(
    amp_factor: u64,
    amount_in_after_fee: u64,
    balance_in: u64,
    balance_out: u64,
) -> Result<u64> {
    let new_balance_in = amount_in_after_fee + balance_in;

    // compute the new ouput balance and ensure that it has decreased in this exchange
    let prev_balance_in = balance_in;
    let prev_balance_out = balance_out;

    let (new_balance_out, _prev_invariant) = get_Y_with_prev_invariant(
        prev_balance_in,
        prev_balance_out,
        new_balance_in,
        amp_factor,
    )?;

    if prev_balance_out > new_balance_out {
        Ok(prev_balance_out - new_balance_out)
    } else {
        Err(anyhow!(
            "invalid prev balance out {}, new balance out {}",
            prev_balance_out,
            new_balance_out
        ))
    }
}

fn get_Y_with_prev_invariant(
    balance_in: u64,
    balance_out: u64,
    new_balance_in: u64,
    amp_factor: u64,
) -> Result<(u64, U256)> {
    let d = compute_invariant(balance_in, balance_out, amp_factor)?;

    let num_coins = 2;
    let n = U256::try_from(num_coins)?;
    let ann = U256::try_from(amp_factor)? * n;

    let mut c: U256 = d;
    let mut s: U256 = *CONST_ZERO;

    let x_k = U256::try_from(new_balance_in)?;
    s = s + x_k;
    c = c * d / (x_k * n);

    c = c * d / n / ann;
    let b = d / ann + s;
    let mut y = d;
    let mut k = 0;

    while (k < MAX_LOOP_LIMIT) {
        let prev_y = y;
        y = (y * y + c) / (*CONST_TWO * y + b - d);

        if (abs_diff(y, prev_y) <= *CONVERGE_THRESHOLD) {
            // return (y + 10) instead of y because numerical errors may cause y to be slightly less than what it should be
            // and that would cause the invariant to decrease slightly, and fail the assertion in `get_D`
            // pool precision is 8 digits, so this is equivalent to charging a fee of 0.0000001, which is negligible
            let y_out = y + 10;
            return Ok((y_out.as_u64(), d));
        };

        k = k + 1;
    }

    Err(anyhow!("ERR_STABLE_MATH_Y_NOT_CONVERGE"))
}

/// Compute the invariant, D, given the token balances and amplification factor
///
/// Based off of the Balancer V2 Stable Pool Invariant:
/// ********************************************************************************************//
/// D = invariant                                                                               //
/// A = amplification coefficient                                         D^(n+1)               //
/// S = sum of balances                A * n^n * S + D = A * D * n^n + -------------            //
/// P = product of balances                                                n^n P                //
/// n = number of tokens                                                                        //
/// ********************************************************************************************//
///
/// The invariant is solved for using the Newton Raphson Method. See
/// https://atulagarwal.dev/posts/curveamm/stableswap for an analysis of the approximation
///
/// *********************************************************************************************//
///                                                                      |                       //
/// D  = next estimated invariant                D' (S*An^n + n*Dp)      |          D'^(n+1)     //
/// D' = previously estimated invariant    D = ------------------------  |   Dp = ------------   //
///                                             D'*(An^n - 1) + (n+1)Dp  |           P n^n       //
///                                                                      |                       //
/// *********************************************************************************************//
///
/// NOTE: In practice, we encode the An^n constant as An for simplicity. Many implementations of
/// this invariant do the same.
///
/// a: amplification factor
/// xp: normalized balances of tokens in the pool
pub fn compute_invariant(balance_in: u64, balance_out: u64, amp_factor: u64) -> Result<U256> {
    let num_coins = 2;

    let balance_in = U256::try_from(balance_in)?;
    let balance_out = U256::try_from(balance_out)?;

    let s: U256 = balance_in + balance_out;

    if (s == *CONST_ZERO) {
        return Ok(*CONST_ZERO);
    };

    let mut d = s;
    let ann = U256::try_from(amp_factor * num_coins)?;

    let mut n = *CONST_TWO;

    let p = balance_in * balance_out;

    let mut i = 0;
    while (i < MAX_LOOP_LIMIT) {
        let prev_d = d;
        let d_squared = d * d;

        // d^3 / n^2 / p, where n^2 = 4
        let dp = d * d_squared / p / 4;

        d = (ann * s + n * dp) * d / ((ann - 1) * d + dp * (n + 1));

        if (abs_diff(prev_d, d) <= *CONVERGE_THRESHOLD) {
            return Ok(d);
        };

        i = i + 1;
    }

    Err(anyhow!("ERR_STABLE_MATH_D_NOT_CONVERGE"))
}

fn abs_diff(x: U256, y: U256) -> U256 {
    if (x >= y) {
        x - y
    } else {
        y - x
    }
}
