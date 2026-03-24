mod values;
use crate::swap::amm::CoinSwapDirection;
use serde::{Deserialize, Serialize};
use values::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObricAmmParam {
    pub k: u128,
    pub k2: u128,
    pub m: u64,
    pub n: u64,
    pub xa: u64,
    pub xb: u64,
    pub x_deci_mult: u64,
    pub y_deci_mult: u64,
}

pub fn get_obric_swap_coin_out_with_fee(
    coin_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    fee_rate: u64,
    fee_scale: u64,
    param: &ObricAmmParam,
    swap_direction: &CoinSwapDirection,
) -> u64 {
    debug_assert!(coin_in != 0);
    let output_amount =
        get_obric_swap_coin_out(coin_in, reserve_in, reserve_out, param, swap_direction);

    output_amount - (output_amount * fee_rate / fee_scale)
}

pub fn get_obric_swap_coin_out(
    coin_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    param: &ObricAmmParam,
    swap_direction: &CoinSwapDirection,
) -> u64 {
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

    let (reserve_in, reserve_out, in_deci_mult, out_deci_mult) = {
        match swap_direction {
            CoinSwapDirection::XToY => (
                UnPrecisedReserveIn::new(reserve_in as u128 * *x_deci_mult as u128),
                UnPrecisedReserveOut::new(reserve_out as u128 * *y_deci_mult as u128),
                *x_deci_mult,
                *y_deci_mult,
            ),
            CoinSwapDirection::YToX => (
                UnPrecisedReserveIn::new(reserve_in as u128 * *y_deci_mult as u128),
                UnPrecisedReserveOut::new(reserve_out as u128 * *x_deci_mult as u128),
                *y_deci_mult,
                *x_deci_mult,
            ),
        }
    };

    let coin_in = coin_in as u128 * in_deci_mult as u128;
    let k = UnPrecisedK::new(*k as u128);
    let k2 = UnPrecisedK2::new(*k2 as u128);
    let xa = UnPrecisedXa::new(*xa as u128);
    let xb = UnPrecisedXb::new(*xb as u128);
    let m = UnPrecisedM::new(*m as u128);
    let n = UnPrecisedN::new(*n as u128);

    let preprocessing_normalizer = {
        let reserve_max = reserve_in.max_amount_with(&reserve_out);
        // normlize sig_digit = 10^9
        calc_significant_digit(reserve_max, 9)
    };

    let reserve_in = reserve_in.normalize(&preprocessing_normalizer);
    let reserve_out = reserve_out.normalize(&preprocessing_normalizer);

    let input_amount = if preprocessing_normalizer.normalize(coin_in) < 1000 {
        InputAmount::new_not_normalized(coin_in)
    } else {
        InputAmount::new_normalized(coin_in)
    };

    let normalized_output = get_swap_in_to_out(
        input_amount, //TODO(change)
        reserve_in,
        reserve_out,
        k,
        k2,
        xa,
        xb,
        m,
        n,
        &preprocessing_normalizer,
    );
    let actual_output = normalized_output
        .un_normalize(&preprocessing_normalizer)
        .un_precise() as u64
        / out_deci_mult;
    actual_output
}

macro_rules! calc_f_reserve_values {
    ($current_reserve_in:expr, $current_reserve_out: expr, $input_amount:expr, $f_normalizer:expr, $preprocessing_normalizer:expr) => {{
        let current_reserve_in_f = $current_reserve_in.normalize($f_normalizer).prcise();
        let current_reserve_out_f = $current_reserve_out.normalize($f_normalizer).prcise();

        let normalized_input_amount_f = {
            let f_normalized_input_amount =
                $input_amount.update(|amount| $f_normalizer.normalize(amount));
            let normalized_input_amount =
                f_normalized_input_amount.normalize_if_need($preprocessing_normalizer);
            normalized_input_amount.precise()
        };

        let new_reserve_in_with_input_f =
            current_reserve_in_f.add_input_amount(normalized_input_amount_f);
        (current_reserve_out_f, new_reserve_in_with_input_f)
    }};
}

macro_rules! calc_f_output_by_k2 {
    ($k2:expr, $new_reserve_in_with_input_f: expr, $current_reserve_out_f:expr, $n:expr, $f_normalizer:expr) => {{
        debug!(
            "========k2:{:?} x:{:?} n:{:?}",
            $k2.precise(),
            $new_reserve_in_with_input_f,
            $n.precise(),
        );

        // p_new_yF__27 = $.copy(p_k2).div($.copy(p_new_xF__26).sub($.copy(p_n)));
        let new_reserve_out_f = $k2
            .precise()
            .div($new_reserve_in_with_input_f.sub_n($n.precise()));

        let new_reserve_out_f_delta = $current_reserve_out_f.sub_or_zero(&new_reserve_out_f);

        PrecisedNormalizedOutputAmount::new(new_reserve_out_f_delta.un_normalize(&$f_normalizer))
    }};
}

macro_rules! calc_f_output_by_k {
    ($k:expr, $new_reserve_in_with_input_f:expr, $current_reserve_out_f:expr, $m:expr, $f_normalizer:expr) => {{
        let precised_m = $m.precise();
        let new_reserve_out_f = $k
            .precise()
            .div($new_reserve_in_with_input_f.add_m(precised_m))
            .sub_m(precised_m);

        let new_reserve_out_f_delta = $current_reserve_out_f.sub_or_zero(&new_reserve_out_f);
        PrecisedNormalizedOutputAmount::new(new_reserve_out_f_delta.un_normalize(&$f_normalizer))
    }};
}

fn get_swap_in_to_out(
    input_amount: InputAmount,

    current_reserve_in: UnPrecisedReserveIn,
    current_reserve_out: UnPrecisedReserveOut,

    k: UnPrecisedK,
    k2: UnPrecisedK2,

    xa: UnPrecisedXa,
    xb: UnPrecisedXb,
    m: UnPrecisedM,
    n: UnPrecisedN,

    preprocessing_normalizer: &Normalizer,
) -> PrecisedNormalizedOutputAmount {
    let reseve_in_out_ratio =
        Ratio::with_unprecised_reserves(current_reserve_in, current_reserve_out);
    let xa_xb_ratio = Ratio::with_xa_xb(xa, xb);

    if reseve_in_out_ratio < xa_xb_ratio {
        calc_swap_output_left_or_middle(
            input_amount,
            current_reserve_in,
            current_reserve_out,
            k,
            k2,
            xa,
            xb,
            m,
            n,
            &preprocessing_normalizer,
            ResolverType::UpperLeft,
        )
    } else {
        let xb_xa_ratio = Ratio::with_xb_xa(xb, xa);
        if reseve_in_out_ratio < xb_xa_ratio {
            let xb = xa.as_xb();
            let xa = xb.as_xa();

            calc_swap_output_left_or_middle(
                input_amount,
                current_reserve_in,
                current_reserve_out,
                k,
                k2,
                xa,
                xb,
                m,
                n,
                &preprocessing_normalizer,
                ResolverType::Middle,
            )
        } else {
            calc_swap_output_bottom_right(
                input_amount,
                current_reserve_in,
                current_reserve_out,
                k,
                k2,
                xa,
                xb,
                m,
                n,
                &preprocessing_normalizer,
            )
        }
    }
}

enum ResolverType {
    UpperLeft,
    Middle,
}

fn calc_swap_output_left_or_middle(
    input_amount: InputAmount,

    current_reserve_in: UnPrecisedReserveIn,
    current_reserve_out: UnPrecisedReserveOut,

    k: UnPrecisedK,
    k2: UnPrecisedK2,

    xa: UnPrecisedXa,
    xb: UnPrecisedXb,
    m: UnPrecisedM,
    n: UnPrecisedN,

    preprocessing_normalizer: &Normalizer,
    resolver_type: ResolverType,
) -> PrecisedNormalizedOutputAmount {
    let (f_normalizer, dydx_normalizer) = match resolver_type {
        ResolverType::UpperLeft => {
            solve_f_upper_left(&current_reserve_in, &current_reserve_out, &n, &k2)
        }
        ResolverType::Middle => solve_f_middle(&current_reserve_in, &current_reserve_out, &m, &k),
    };

    let (current_reserve_out_f, new_reserve_in_with_input_f) = calc_f_reserve_values!(
        current_reserve_in,
        current_reserve_out,
        input_amount,
        &f_normalizer,
        &preprocessing_normalizer
    );
    let precised_xa = xa.precise();
    let precised_xb = xb.precise();

    if new_reserve_in_with_input_f.is_greater_than_xa(&precised_xa) {
        let max_output_amount = {
            let normalized_input_amount = input_amount.normalize_if_need(&preprocessing_normalizer);
            let normalized_input_amount = normalized_input_amount.precise();

            let max_output_amount = normalized_input_amount
                .precised_amount()
                .normalize(&dydx_normalizer);
            PrecisedNormalizedOutputAmount::new(max_output_amount)
        };

        let delta_out_this_stage = current_reserve_out_f.sub_xb(precised_xb);
        let next_stage_input = new_reserve_in_with_input_f
            .sub_xa(precised_xa)
            .un_presiced()
            .as_normalized_input_amount();

        let next_stage_reserve_in = xa.convert_to_reserve_in();
        let next_stage_reserve_out = xb.convert_to_reserve_out();

        let output_amount_next_stage = get_swap_in_to_out(
            next_stage_input,
            next_stage_reserve_in,
            next_stage_reserve_out,
            k,
            k2,
            xa,
            xb,
            m,
            n,
            &Normalizer::default(),
        );

        let precised_output = output_amount_next_stage
            .add_delta(&delta_out_this_stage)
            .un_normalize(&f_normalizer);

        precised_output.min(&max_output_amount)
    } else {
        match resolver_type {
            ResolverType::UpperLeft => calc_f_output_by_k2!(
                k2,
                new_reserve_in_with_input_f,
                current_reserve_out_f,
                n,
                f_normalizer
            ),

            ResolverType::Middle => calc_f_output_by_k!(
                k,
                new_reserve_in_with_input_f,
                current_reserve_out_f,
                m,
                f_normalizer
            ),
        }
    }
}

fn calc_swap_output_bottom_right(
    input_amount: InputAmount,

    current_reserve_in: UnPrecisedReserveIn,
    current_reserve_out: UnPrecisedReserveOut,

    _k: UnPrecisedK,
    k2: UnPrecisedK2,

    _xa: UnPrecisedXa,
    _xb: UnPrecisedXb,
    _m: UnPrecisedM,
    n: UnPrecisedN,

    preprocessing_normalizer: &Normalizer,
) -> PrecisedNormalizedOutputAmount {
    let (f_normalizer, _) =
        solve_f_bottom_right(&current_reserve_in, &current_reserve_out, &n, &k2);

    let (current_reserve_out_f, new_reserve_in_with_input_f) = calc_f_reserve_values!(
        current_reserve_in,
        current_reserve_out,
        input_amount,
        &f_normalizer,
        &preprocessing_normalizer
    );

    calc_f_output_by_k2!(
        k2,
        new_reserve_in_with_input_f,
        current_reserve_out_f,
        n,
        f_normalizer
    )
}

fn solve_f_upper_left(
    reserve_in: &UnPrecisedReserveIn,
    reserve_out: &UnPrecisedReserveOut,
    n: &UnPrecisedN,
    k2: &UnPrecisedK2,
) -> (Normalizer, Normalizer) {
    let x = reserve_in.un_precised_amount();
    let y = reserve_out.un_precised_amount();
    let xy = x * y;
    let n = n.un_precised_amount();
    let xn = x * n;
    let k2 = k2.un_precised_amount();

    // xn + (xn ^2  + 4 * xy *k2)^1/2
    let f_numerator = (xn) + (xn.pow(2) + xy * k2 * 4).sqrt();
    let f_denom = xy * 2;
    let f_normalizer = Normalizer::new(f_numerator.amount(), f_denom.amount());

    let dxdy_denom = f_normalizer.normalize(x.amount()).pow(2);
    (f_normalizer, Normalizer::new(k2.amount(), dxdy_denom))
}

fn solve_f_middle(
    reserve_in: &UnPrecisedReserveIn,
    reserve_out: &UnPrecisedReserveOut,
    m: &UnPrecisedM,
    k: &UnPrecisedK,
) -> (Normalizer, Normalizer) {
    let x = reserve_in.un_precised_amount();
    let y = reserve_out.un_precised_amount();
    let m = m.un_precised_amount();
    let k = k.un_precised_amount();

    let xy = x * y;
    let x_plus_y = x + y;
    let b = x_plus_y * m;

    // num = ( (b ^ 2 ) + (4 * xy * ( k - m ^ 2 ) )   ) ^ 1/2 - b
    let f_numerator = (b.pow(2) + (xy * (k - (m.pow(2))) * 4)).sqrt() - b;
    // denom + 2 * xy
    let f_denominator = xy * 2;
    let f_normalizer = Normalizer::new(f_numerator.amount(), f_denominator.amount());

    //  // x_f = norm(x)
    let x_f = x.normalize(&f_normalizer);

    // x_f_m  = x_f + m
    let x_f_plus_m = x_f + m;

    (
        f_normalizer,
        Normalizer::new(k.amount(), x_f_plus_m.pow(2).amount()),
    )
}

fn solve_f_bottom_right(
    reserve_in: &UnPrecisedReserveIn,
    reserve_out: &UnPrecisedReserveOut,
    n: &UnPrecisedN,
    k2: &UnPrecisedK2,
) -> (Normalizer, Normalizer) {
    solve_f_upper_left(
        &reserve_out.as_reserve_in(),
        &reserve_in.as_reserve_out(),
        n,
        k2,
    )
}

fn calc_significant_digit(mut base_value: u128, significant_digit: u32) -> Normalizer {
    let significant_digit_threshold = 10u128.pow(significant_digit);
    let mut denominator = 1u128;
    while base_value > significant_digit_threshold {
        base_value = base_value / 10;
        denominator = denominator * 10;
    }

    let mut numerator = 1u128;

    while base_value < significant_digit_threshold {
        base_value = base_value * 10;
        numerator = numerator * 10;
    }

    Normalizer::new(numerator, denominator)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_obric_calc_sig_digit() {
        {
            let sig = calc_significant_digit(100_000_000, 9);
            assert_eq!(Normalizer::new(10, 1), sig);

            assert_eq!(1_000_000_000, sig.normalize(100_000_000));
        }

        {
            let sig = calc_significant_digit(1_000_000_000, 9);
            assert_eq!(Normalizer::new(1, 1), sig);

            assert_eq!(1_000_000_000, sig.normalize(1_000_000_000));
        }

        {
            let sig = calc_significant_digit(30_000_000_000, 9);
            assert_eq!(Normalizer::new(10, 100), sig);

            assert_eq!(3_000_000_000, sig.normalize(30_000_000_000));
        }
    }

    #[test]
    fn test_obric_get_amount_1() {
        //     // before
        //          "data": {
        //            "K": "1000000000000000000",
        //            "K2": "2195873881596",
        //            "Xa": "1474494",
        //            "Xb": "11424866",
        //            "lp_amt": "5000000000",
        //            "lp_burn_cap": {
        //              "dummy_field": false
        //            },
        //            "lp_freeze_cap": {
        //              "dummy_field": false
        //            },
        //            "lp_mint_cap": {
        //              "dummy_field": false
        //            },
        //            "m": "993562696",
        //            "n": "9935628",
        //            "protocol_fee_share_per_thousand": "300",
        //            "protocol_fee_x": {
        //              "value": "60250"
        //            },
        //            "protocol_fee_y": {
        //              "value": "30030"
        //            },
        //            "reserve_x": {
        //              "value": "4495490847"
        //            },
        //            "reserve_y": {
        //              "value": "5505048009"
        //            },
        //            "swap_fee_per_million": "200",
        //            "x_deci_mult": "1",
        //            "y_deci_mult": "1"
        //          }
        //

        //  tx
        //    "tx_hash": "0x9e06428f3fab3a23c771e3e4054a90d75006806b06080508757a0eb683f22575",
        //    "version": 30766349,
        //    "gas_used": 1873,
        //     // after
        //      {
        //        "guid": {
        //          "creation_number": "4",
        //          "account_address": "0x89576037b3cc0b89645ea393a47787bb348272c76d6941c574b053672b848039"
        //        },
        //        "sequence_number": "294161",
        //        "type": "0x89576037b3cc0b89645ea393a47787bb348272c76d6941c574b053672b848039::aggregator::SwapStepEvent",
        //        "data": {
        //          "dex_type": 12,
        //          "input_amount": "1005484810",
        //          "output_amount": "1003984034",
        //          "pool_type": "0",
        //          "time_stamp": "1668394972855572",
        //          "x_type_info": {
        //            "account_address": "0x1",
        //            "module_name": "0x636f696e",
        //            "struct_name": "0x436f696e3c3078613265646132316135383835366664613836343531343336353133623836376339376565636234626130393964613537373535323065306637343932653835323a3a636f696e3a3a543e"
        //          },
        //          "y_type_info": {
        //            "account_address": "0x1",
        //            "module_name": "0x636f696e",
        //            "struct_name": "0x436f696e3c3078356531353666313230376430656266613139613965656666303064363261323832323738666238373139663466616233613538366130613263306666666265613a3a636f696e3a3a543e"
        //          }
        //        }

        //      after
        //        "data": {
        //          "type": "0xc7ea756470f72ae761b7986e4ed6fd409aad183b1b2d3d2f674d979852f45c4b::piece_swap::PieceSwapPoolInfo<0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea::coin::T, 0xa2eda21a58856fda86451436513b867c97eecb4ba099da5775520e0f7492e852::coin::T>",
        //          "data": {
        //            "K": "1000000000000000000",
        //            "K2": "2195873881596",
        //            "Xa": "1474494",
        //            "Xb": "11424866",
        //            "lp_amt": "5000000000",
        //            "lp_burn_cap": {
        //              "dummy_field": false
        //            },
        //            "lp_freeze_cap": {
        //              "dummy_field": false
        //            },
        //            "lp_mint_cap": {
        //              "dummy_field": false
        //            },
        //            "m": "993562696",
        //            "n": "9935628",
        //            "protocol_fee_share_per_thousand": "300",
        //            "protocol_fee_x": {
        //              "value": "60250"
        //            },
        //            "protocol_fee_y": {
        //              "value": "0"
        //            },
        //            "reserve_x": {
        //              "value": "3995955716"
        //            },
        //            "reserve_y": {
        //              "value": "6005484810"
        //            },
        //            "swap_fee_per_million": "200",
        //            "x_deci_mult": "1",
        //            "y_deci_mult": "1"
        //          }
        //

        //get_obric_swap_coin_out_with_fee()

        //          "input_amount": "1005484810",
        //          "output_amount": "1003984034",

        let coin_in = 1005484810;
        let reserve_in = 4495490847;
        let reserve_out = 5505048009;
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
                x_deci_mult: 1,
                y_deci_mult: 1,
            },
            &CoinSwapDirection::XToY,
        );
        assert_eq!(actual, 1003984034);
    }
}
