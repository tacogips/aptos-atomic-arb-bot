use super::fixed_point64::{self, *};

pub fn calc_out_given_in_weights_u64(
    amount_in_after_fee: u64,
    balance_in: u64,
    balance_out: u64,
    weight_in: u64,
    weight_out: u64,
) -> u64 {
    let weight_ratio = fixed_point64::fraction(weight_in, weight_out);

    let amount_out =
        calc_out_given_in_internal(balance_in, balance_out, amount_in_after_fee, weight_ratio);

    amount_out
}

fn calc_out_given_in_internal(
    balance_in: u64,
    balance_out: u64,
    aI: u64,
    weight_ratio: FixedPoint64,
) -> u64 {
    let fraction = fixed_point64::fraction(balance_in, balance_in + aI);

    // Note: fixed_point64::sub_fp might panic on underflow
    // result is smaller than or equal to the actual value
    let result = fixed_point64::mul(
        fixed_point64::sub_fp(
            fixed_point64::one(),
            log_exp_math::pow_up(fraction, weight_ratio),
        ),
        balance_out,
    );
    fixed_point64::decode_round_down(result)
}
