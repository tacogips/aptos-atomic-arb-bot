#![allow(clippy::style)]
#![allow(non_snake_case)]
#![allow(unused_parens)]
#![allow(unused_mut)]
#![allow(unused_assignments)]

use aptos_logger::prelude::*;
use crate::swap::amm::CoinSwapDirection;
use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

const BILLION: u128 = 1000000000;
const PRECISION_FACTOR: u128 = 1000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObricAmmParam {
    #[serde(deserialize_with = "from_u128_string")]
    #[serde(serialize_with = "serialize_as_str")]
    pub k: u128,

    #[serde(deserialize_with = "from_u128_string")]
    #[serde(serialize_with = "serialize_as_str")]
    pub k2: u128,
    pub m: u64,
    pub n: u64,
    pub xa: u64,
    pub xb: u64,
    pub x_deci_mult: u64,
    pub y_deci_mult: u64,
}

fn from_u128_string<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr,
    D: Deserializer<'de>,
    <T as FromStr>::Err: std::fmt::Display,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt<T> {
        String(String),
        Number(T),
    }

    match StringOrInt::<T>::deserialize(deserializer)? {
        StringOrInt::String(s) => s.parse::<T>().map_err(serde::de::Error::custom),
        StringOrInt::Number(i) => Ok(i),
    }
}

pub fn serialize_as_str<T, S>(value: T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: ToString,
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

pub fn get_obric_swap_coin_out_with_fee(
    coin_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    fee_rate: u64,
    _fee_scale: u64,
    param: &ObricAmmParam,
    swap_direction: &CoinSwapDirection,
) -> Result<u64> {
    debug_assert!(coin_in != 0);

    let ObricAmmParam {
        k,
        k2,
        m,
        n,
        xa,
        xb,
        x_deci_mult,
        y_deci_mult,
    } = param;

    let output = match swap_direction {
        CoinSwapDirection::XToY => quote_x_to_y_after_fees_(
            reserve_in as u128,
            reserve_out as u128,
            *x_deci_mult as u128,
            *y_deci_mult as u128,
            *k as u128,
            *k2 as u128,
            *xa as u128,
            *xb as u128,
            *m as u128,
            *n as u128,
            coin_in as u128,
            fee_rate as u128,
        )?,
        CoinSwapDirection::YToX => quote_y_to_x_after_fees_(
            reserve_out as u128, // reseve_out == reserve_x
            reserve_in as u128,  // reseve_in == reserve_y
            *x_deci_mult as u128,
            *y_deci_mult as u128,
            *k as u128,
            *k2 as u128,
            *xa as u128,
            *xb as u128,
            *m as u128,
            *n as u128,
            coin_in as u128,
            fee_rate as u128,
        )?,
    };
    Ok(output as u64)
}

fn max(a: u128, b: u128) -> u128 {
    a.max(b)
}

fn min_(a: u128, b: u128) -> u128 {
    if a > b {
        return b;
    }
    return a;
}

fn mul_w_(multiplier: u128, numerator: u128, denominator: u128) -> u128 {
    match multiplier.checked_mul(numerator) {
        None => 0,
        Some(val) => val / denominator,
    }
}

fn get_swap_x_to_y_out_preprocessed_(
    current_x: u128,
    current_y: u128,
    input_x: u128,
    k: u128,
    k2: u128,
    xa: u128,
    xb: u128,
    m: u128,
    n: u128,
) -> Result<u128> {
    return get_swap_x_to_y_out_preprocessed_inner_(
        current_x, current_y, input_x, 1, 1, k, k2, xa, xb, m, n,
    );
}

fn quote_x_to_y_after_fees_(
    reserve_x: u128,
    reserve_y: u128,
    x_deci_mult: u128,
    y_deci_mult: u128,
    K: u128,
    K2: u128,
    Xa: u128,
    Xb: u128,
    m: u128,
    n: u128,
    amount_x_in: u128,
    swap_fee_per_million: u128,
) -> Result<u128> {
    let actual_out_y = quote_x_to_y_(
        reserve_x,
        reserve_y,
        x_deci_mult,
        y_deci_mult,
        K,
        K2,
        Xa,
        Xb,
        m,
        n,
        amount_x_in,
    )?;
    let total_fees = actual_out_y * swap_fee_per_million / 1000000;
    let out_y_after_fees = actual_out_y - total_fees;
    return Ok(out_y_after_fees);
}

fn quote_y_to_x_after_fees_(
    reserve_x: u128,
    reserve_y: u128,
    x_deci_mult: u128,
    y_deci_mult: u128,
    K: u128,
    K2: u128,
    Xa: u128,
    Xb: u128,
    m: u128,
    n: u128,
    amount_y_in: u128,
    swap_fee_per_million: u128,
) -> Result<u128> {
    let actual_out_y = quote_y_to_x_(
        reserve_x,
        reserve_y,
        x_deci_mult,
        y_deci_mult,
        K,
        K2,
        Xa,
        Xb,
        m,
        n,
        amount_y_in,
    )?;
    let total_fees = actual_out_y * swap_fee_per_million / 1000000;
    let out_y_after_fees = actual_out_y - total_fees;
    return Ok(out_y_after_fees);
}

fn quote_x_to_y_(
    reserve_x: u128,
    reserve_y: u128,
    x_deci_mult: u128,
    y_deci_mult: u128,
    K: u128,
    K2: u128,
    Xa: u128,
    Xb: u128,
    m: u128,
    n: u128,
    amount_x_in: u128,
) -> Result<u128> {
    let mut current_x = reserve_x * x_deci_mult;
    let mut current_y = reserve_y * y_deci_mult;
    let mut x_value = amount_x_in;

    let mut input_x = x_value * x_deci_mult;

    let opt_output_y = get_swap_x_to_y_out_(current_x, current_y, input_x, K, K2, Xa, Xb, m, n)?;
    let mut actual_out_y = opt_output_y / y_deci_mult;
    return Ok(actual_out_y);
}

fn quote_y_to_x_(
    reserve_x: u128,
    reserve_y: u128,
    x_deci_mult: u128,
    y_deci_mult: u128,
    K: u128,
    K2: u128,
    Xa: u128,
    Xb: u128,
    m: u128,
    n: u128,
    amount_y_in: u128,
) -> Result<u128> {
    let mut current_x = reserve_x * x_deci_mult;
    let mut current_y = reserve_y * y_deci_mult;

    let y_value = amount_y_in;
    let input_y = y_value * y_deci_mult;

    let opt_output_x = get_swap_y_to_x_out_(current_x, current_y, input_y, K, K2, Xa, Xb, m, n)?;
    let actual_out_x = opt_output_x / x_deci_mult;
    return Ok(actual_out_x);
}

fn sqrt_(y: u128) -> u128 {
    if y < 4 {
        let temp_1 = if y == 0 { 0 } else { 1 };

        temp_1
    } else {
        let mut z = y;
        let mut x = y / 2 + 1;
        while x < z {
            z = x;
            x = (y / x + x) / 2;
        }
        z
    }
}

fn solve_F_upper_left_(x: u128, y: u128, n: u128, k2: u128) -> (u128, u128, u128, u128) {
    let xn = x * n;
    let xy = x * y;
    let numerator = xn + sqrt_(xn * xn + (4 * xy * k2));
    let denominator = 2 * xy;
    let xF = mul_w_(x, numerator, denominator);
    return (numerator, denominator, k2, xF * xF);
}

fn solve_F_bottom_right_(x: u128, y: u128, n: u128, k2: u128) -> (u128, u128, u128, u128) {
    return solve_F_upper_left_(y, x, n, k2);
}

fn solve_F_middle_(x: u128, y: u128, m: u128, k: u128) -> (u128, u128, u128, u128) {
    let xy = x * y;
    let x_plus_y = x + y;
    let b = x_plus_y * m;
    let numerator = sqrt_(b * b + (4 * xy * (k - (m * m)))) - b;
    let denominator = 2 * xy;
    let xF = mul_w_(x, numerator, denominator);
    let xf_plus_m = xF + m;
    return (numerator, denominator, k, xf_plus_m * xf_plus_m);
}

fn compare_fraction_(
    first_numerator: u128,
    first_denominator: u128,
    second_numerator: u128,
    second_denominator: u128,
) -> bool {
    return first_numerator * second_denominator < second_numerator * first_denominator;
}

fn get_swap_x_to_y_out_(
    current_x: u128,
    current_y: u128,
    input_x: u128,
    k: u128,
    k2: u128,
    xa: u128,
    xb: u128,
    m: u128,
    n: u128,
) -> Result<u128> {
    let mut max_x_y = max(current_x, current_y);
    let mut numerator = 1;
    let mut denominator = 1;

    while max_x_y > BILLION {
        max_x_y = max_x_y / 10;
        denominator = denominator * 10;
    }
    while max_x_y < BILLION / 10 {
        max_x_y = max_x_y * 10;
        numerator = numerator * 10;
    }

    let preprocessed_input_x = input_x * numerator / denominator;

    if preprocessed_input_x < 10000 {
        Ok((get_swap_x_to_y_out_preprocessed_inner_(
            (current_x * numerator / denominator),
            (current_y * numerator / denominator),
            input_x,
            numerator,
            denominator,
            k,
            k2,
            xa,
            xb,
            m,
            n,
        )? * denominator
            / (numerator * PRECISION_FACTOR)))
    } else {
        Ok((get_swap_x_to_y_out_preprocessed_inner_(
            (current_x * numerator / denominator),
            (current_y * numerator / denominator),
            preprocessed_input_x,
            1,
            1,
            k,
            k2,
            xa,
            xb,
            m,
            n,
        )? * denominator
            / (numerator * PRECISION_FACTOR)))
    }
}

fn get_swap_y_to_x_out_(
    current_x: u128,
    current_y: u128,
    input_y: u128,
    k: u128,
    k2: u128,
    xa: u128,
    xb: u128,
    m: u128,
    n: u128,
) -> Result<u128> {
    return get_swap_x_to_y_out_(current_y, current_x, input_y, k, k2, xa, xb, m, n);
}

fn get_swap_x_to_y_out_preprocessed_inner_(
    current_x: u128,
    current_y: u128,
    input_x: u128,
    preprocessing_numerator: u128,
    preprocessing_denominator: u128,
    k: u128,
    k2: u128,
    xa: u128,
    xb: u128,
    m: u128,
    n: u128,
) -> Result<u128> {
    let p_xa = xa * PRECISION_FACTOR;
    let p_xb = xb * PRECISION_FACTOR;
    let p_m = m * PRECISION_FACTOR;
    let p_n = n * PRECISION_FACTOR;
    let p_k = k * PRECISION_FACTOR * PRECISION_FACTOR;
    let p_k2 = k2 * PRECISION_FACTOR * PRECISION_FACTOR;

    let mut temp_32 = 0;

    if compare_fraction_(current_x, current_y, xa, xb) {
        let (f_numerator, f_denominator, dydx_numerator, dydx_denominator) =
            solve_F_upper_left_(current_x, current_y, n, k2);

        let p_current_xF = current_x * f_numerator * PRECISION_FACTOR / f_denominator;
        let p_current_yF = current_y * f_numerator * PRECISION_FACTOR / f_denominator;

        let p_input_xF = (input_x * f_numerator * PRECISION_FACTOR / f_denominator
            * preprocessing_numerator
            / preprocessing_denominator);
        let p_new_xF = p_current_xF + p_input_xF;

        let mut temp_3 = 0;
        if p_new_xF > p_xa {
            let p_output_y_max = mul_w_(
                input_x * PRECISION_FACTOR * preprocessing_numerator / preprocessing_denominator,
                dydx_numerator,
                dydx_denominator,
            );

            let p_delta_yF_this_stage = p_current_yF - p_xb;

            let input_xF_next_stage = (p_new_xF - p_xa) / PRECISION_FACTOR;

            let p_output_yF_next_stage = get_swap_x_to_y_out_preprocessed_(
                xa,
                xb,
                input_xF_next_stage,
                k,
                k2,
                xa,
                xb,
                m,
                n,
            )?;

            let p_output_y =
                (p_delta_yF_this_stage + p_output_yF_next_stage * f_denominator / f_numerator);
            temp_3 = min_(p_output_y, p_output_y_max);
        } else {
            let p_new_yF = p_k2 / p_new_xF + p_n;
            let mut temp_1 = 0;
            if p_current_yF > p_new_yF {
                temp_1 = p_current_yF - p_new_yF
            } else {
                temp_1 = 0
            }

            let p_delta_yF = temp_1;
            let p_output_y__2 = p_delta_yF * f_denominator / f_numerator;
            temp_3 = p_output_y__2;
        }

        temp_32 = temp_3
    } else {
        let mut temp_31 = 0;
        if compare_fraction_(current_x, current_y, xb, xa) {
            let (f_numerator__4, f_denominator__5, dydx_numerator__6, dydx_denominator__7) =
                solve_F_middle_(current_x, current_y, m, k);

            let p_current_xF__8 =
                (current_x * f_numerator__4 * PRECISION_FACTOR / f_denominator__5);

            let p_current_yF__9 =
                (current_y * f_numerator__4 * PRECISION_FACTOR / f_denominator__5);

            let p_input_xF__10 = (input_x * f_numerator__4 * PRECISION_FACTOR / f_denominator__5
                * preprocessing_numerator
                / preprocessing_denominator);

            let p_new_xF__11 = p_current_xF__8 + p_input_xF__10;

            let mut temp_20 = 0;
            if p_new_xF__11 > p_xb {
                let p_output_y_max__12 = mul_w_(
                    input_x * PRECISION_FACTOR * preprocessing_numerator
                        / preprocessing_denominator,
                    dydx_numerator__6,
                    dydx_denominator__7,
                );

                let p_delta_yF_this_stage__13 = p_current_yF__9 - p_xa;

                let input_xF_next_stage__14 = (p_new_xF__11 - p_xb) / PRECISION_FACTOR;

                let p_output_yF_next_stage__15 = get_swap_x_to_y_out_preprocessed_(
                    xb,
                    xa,
                    input_xF_next_stage__14,
                    k,
                    k2,
                    xa,
                    xb,
                    m,
                    n,
                )?;

                let p_output_y__16 = ((p_delta_yF_this_stage__13 + p_output_yF_next_stage__15)
                    * f_denominator__5
                    / f_numerator__4);
                temp_20 = min_(p_output_y__16, p_output_y_max__12);
            } else {
                let p_new_yF__17 = p_k / (p_new_xF__11 + p_m) - p_m;
                if p_current_yF__9 < p_new_yF__17 {
                    //TODO fix
                    error!(
                        "p_current_yF__9 < p_new_yF__17 {},{}",
                        p_current_yF__9,
                        p_new_yF__17
                    );
                    return Ok(0);
                }
                let p_delta_yF__18 = p_current_yF__9 - p_new_yF__17; //TODO() panic
                let p_output_y__19 = p_delta_yF__18 * f_denominator__5 / f_numerator__4;
                temp_20 = p_output_y__19
            }

            temp_31 = temp_20
        } else {
            let (f_numerator__21, f_denominator__22, _dydx_numerator, _dydx_denominator) =
                solve_F_bottom_right_(current_x, current_y, n, k2);

            let p_current_xF__23 =
                (current_x * f_numerator__21 * PRECISION_FACTOR / f_denominator__22);
            let p_current_yF__24 =
                (current_y * f_numerator__21 * PRECISION_FACTOR / f_denominator__22);
            let p_input_xF__25 = (input_x * f_numerator__21 * PRECISION_FACTOR / f_denominator__22
                * preprocessing_numerator
                / preprocessing_denominator);
            let p_new_xF__26 = p_current_xF__23 + p_input_xF__25;
            let p_new_yF__27 = p_k2 / (p_new_xF__26 - p_n);

            let mut temp_28 = 0u128;
            if p_current_yF__24 > p_new_yF__27 {
                temp_28 = p_current_yF__24 - p_new_yF__27
            } else {
                temp_28 = 0
            };

            let p_delta_yF__29 = temp_28;
            let p_output_y__30 = p_delta_yF__29 * f_denominator__22 / f_numerator__21;
            temp_31 = p_output_y__30
        }

        temp_32 = temp_31
    }

    return Ok(temp_32);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_obric_get_amount_1() {
        let coin_in = 1005484810;
        let reserve_x = 4495490847;
        let reserve_y = 5505048009;
        let fee_rate = 200;
        let fee_scale = 1_000_000;

        let actual = get_obric_swap_coin_out_with_fee(
            coin_in,
            reserve_x,
            reserve_y,
            fee_rate,
            fee_scale,
            &ObricAmmParam {
                k: 1000000000000000000,
                k2: 2195873881596,
                m: 993562696,
                n: 9935628,
                xa: 1474494,
                xb: 11424866,
                x_deci_mult: 1,
                y_deci_mult: 1,
            },
            &CoinSwapDirection::XToY,
        );
        let expected = 1005288980;
        assert_eq!(actual.unwrap(), expected);

        let actual = get_obric_swap_coin_out_with_fee(
            coin_in,
            reserve_y,
            reserve_x,
            fee_rate,
            fee_scale,
            &ObricAmmParam {
                k: 1000000000000000000,
                k2: 2195873881596,
                m: 993562696,
                n: 9935628,
                xa: 1474494,
                xb: 11424866,
                x_deci_mult: 1,
                y_deci_mult: 1,
            },
            &CoinSwapDirection::YToX,
        );
        let expected = 1002680834;
        assert_eq!(actual.unwrap(), expected);
    }

    #[test]
    fn test_obric_get_amount_2() {
        let coin_in = 122200;
        let reserve_x = 304125183679;
        let reserve_y = 32388735906;
        let fee_rate = 800;
        let fee_scale = 1_000_000;

        let actual = get_obric_swap_coin_out_with_fee(
            coin_in,
            reserve_x,
            reserve_y,
            fee_rate,
            fee_scale,
            &ObricAmmParam {
                k: 1000000000000000000,
                k2: 2217728872000,
                m: 996023841,
                n: 4980119,
                xa: 1485495,
                xb: 6473041,
                x_deci_mult: 4429,
                y_deci_mult: 100000,
            },
            &CoinSwapDirection::XToY,
        );
        let expected = 5426;
        assert_eq!(actual.unwrap(), expected);

        let actual = get_obric_swap_coin_out_with_fee(
            coin_in,
            reserve_y,
            reserve_x,
            fee_rate,
            fee_scale,
            &ObricAmmParam {
                k: 1000000000000000000,
                k2: 2217728872000,
                m: 996023841,
                n: 4980119,
                xa: 1485495,
                xb: 6473041,
                x_deci_mult: 4429,
                y_deci_mult: 100000,
            },
            &CoinSwapDirection::YToX,
        );
        let expected = 2747847;
        assert_eq!(actual.unwrap(), expected);
    }

    #[test]
    fn test_obric_get_amount_3() {
        let current_x: u128 = 102140563;
        let current_y: u128 = 101378312;
        let input_x: u128 = 0;
        let preprocessing_numerator: u128 = 1;
        let preprocessing_denominator: u128 = 1000000;
        let k: u128 = 1000000000000000000;
        let k2: u128 = 2195873881596;
        let xa: u128 = 1474494;
        let xb: u128 = 11424866;
        let m: u128 = 993562696;
        let n: u128 = 9935628;

        let actual = get_swap_x_to_y_out_preprocessed_inner_(
            current_x,
            current_y,
            input_x,
            preprocessing_numerator,
            preprocessing_denominator,
            k,
            k2,
            xa,
            xb,
            m,
            n,
        );
        let expected = 0;
        assert_eq!(actual.unwrap(), expected);
    }

    #[test]
    fn test_obric_get_amount_4() {
        let reserve_x = 103280481499;
        let reserve_y = 100239831193;
        let x_deci_mult = 1000;
        let y_deci_mult = 1000;
        let K = 1000000000000000000;
        let K2 = 2195873881596;
        let Xa = 1474494;
        let Xb = 11424866;
        let m = 993562696;
        let n = 9935628;
        let amount_x_in = 0;
        let swap_fee_per_million = 200;

        let actual = quote_x_to_y_after_fees_(
            reserve_x,
            reserve_y,
            x_deci_mult,
            y_deci_mult,
            K,
            K2,
            Xa,
            Xb,
            m,
            n,
            amount_x_in,
            swap_fee_per_million,
        );
        let expected = 0;
        assert_eq!(actual.unwrap(), expected);
    }

    #[test]
    fn test_obric_get_resolve() {
        let reserve_x = 4495490847;
        let reserve_y = 5505048009;

        let k = 1000000000000000000;
        let k2 = 2195873881596;
        let m = 993562696;
        let n = 9935628;

        let ul = solve_F_upper_left_(reserve_x, reserve_y, n, k2);

        assert_eq!(
            ul,
            (
                91701490829252005,
                49495785873510147246,
                2195873881596,
                69369808953316,
            )
        );

        let ml = solve_F_middle_(reserve_x, reserve_y, m, k);

        assert_eq!(
            ml,
            (
                63722540703487527,
                49495785873510147246,
                1000000000000000000,
                998701106055516964,
            ),
        );

        let bt = solve_F_upper_left_(reserve_x, reserve_y, n, k2);

        assert_eq!(
            bt,
            (
                91701490829252005,
                49495785873510147246,
                2195873881596,
                69369808953316,
            )
        );
    }

    #[test]
    fn test_obric_swap_at_79468254() {
        let coin_in = 100000000;

        let reserve_x = 97697628695;
        let reserve_y = 105857686739;

        let reserve_in = reserve_y;
        let reserve_out = reserve_x;

        let fee_rate = 200;
        let fee_scale = 1_000_000;

        let actual = get_obric_swap_coin_out_with_fee(
            coin_in,
            reserve_in,
            reserve_out,
            fee_rate,
            fee_scale,
            &ObricAmmParam {
                k: 1000000000000000000,
                k2: 2195873881596,
                m: 993562696,
                n: 9935628,
                xa: 1474494,
                xb: 11424866,
                x_deci_mult: 1000,
                y_deci_mult: 1000,
            },
            &CoinSwapDirection::YToX,
        );
        let expected = 99927780;
        assert_eq!(actual.unwrap(), expected);
    }
}
