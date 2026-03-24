use super::*;

// Error codes.

/// When exponent is too large
const ERR_EXPONENT_TOO_LARGE: u64 = 0;

const ONE_RAW: u128 = 1 << 64;
const TWO_RAW: u128 = 1 << 65;
const TWO_POW_2_RAW: u128 = 1 << 66;
const TWO_POW_3_RAW: u128 = 1 << 67;
const TWO_POW_4_RAW: u128 = 1 << 68;
const TWO_POW_5_RAW: u128 = 1 << 69;
const TWO_POW_6_RAW: u128 = 1 << 70;
const TWO_POW_NEG_1_RAW: u128 = 1 << 63;
const TWO_POW_NEG_2_RAW: u128 = 1 << 62;
const TWO_POW_NEG_3_RAW: u128 = 1 << 61;
const TWO_POW_NEG_4_RAW: u128 = 1 << 60;

const EXP_1_RAW: u128 = 50143449209799256676;
const EXP_2_RAW: u128 = 136304026803256390374;
const EXP_4_RAW: u128 = 1007158100559408450779;
const EXP_8_RAW: u128 = 54988969081439155349854;
const EXP_16_RAW: u128 = 163919806582506698216928336;
const EXP_32_RAW: u128 = 1456609517792428400051253459476158;
const EXP_1_OVER_2_RAW: u128 = 30413539329486470297;
const EXP_1_OVER_4_RAW: u128 = 23686088245777032821;
const EXP_1_OVER_8_RAW: u128 = 20902899511243624352;
const EXP_1_OVER_16_RAW: u128 = 19636456851539679197;

const LOG_2_E_INV_RAW: u128 = 12786308645977587712; // 1.0 / log_2(e)

const ONE_PLUS_TEN_EXP_MINUS_9: u128 = 18446744092156295689; // fraction(1000000001, 1000000000)
const ONE_MINUS_TEN_EXP_MINUS_9: u128 = 18446744055262807542; // fraction(999999999, 1000000000)

const PRECISION: u8 = 64; // number of bits in the mantissa

// code reference: https://github.com/dmoulding/log2fix/blob/master/log2fix.c
// algorithm: http://www.claysturner.com/dsp/BinaryLogarithm.pdf
pub fn log2(x: FixedPoint64) -> (u8, FixedPoint64) {
    let mut z = to_u128(x);
    let mut y: u128 = 0;
    let mut y_negative: u128 = 0;
    let mut b: u128 = 1 << (PRECISION - 1);
    let mut i: u8 = 0;
    let mut sign: u8 = 1;

    // normalize input to the range [1,2)
    while (z >= TWO_RAW) {
        z = z >> 1;
        y = y + ONE_RAW;
    }

    while (z < ONE_RAW) {
        sign = 0;
        z = z << 1;
        y_negative = y_negative + ONE_RAW;
    }

    while (i < 62) {
        // to calculate (z*z) >> 64, use the fact that z is in the range [1,2)
        // (z >> 1) can fill in lower 64 bits of u128
        // therefore, (z >> 1) * (z >> 1) will not overflow
        z = ((z >> 1) * (z >> 1)) >> 62;
        if (z >= TWO_RAW) {
            z = z >> 1;
            y = y + b;
        };
        b = b >> 1;
        i = i + 1;
    }

    let result = if (sign > 0) {
        from_u128(y)
    } else {
        from_u128(y_negative - y)
    };

    (sign, result)
}

pub fn ln(x: FixedPoint64) -> (u8, FixedPoint64) {
    // ln(x) = log_2(x) / log_2(e)
    let (sign, mut result) = log2(x);
    result = mul_fp(result, from_u128(LOG_2_E_INV_RAW));
    (sign, result)
}

pub fn exp(sign: u8, mut x: FixedPoint64) -> FixedPoint64 {
    debug_assert!(to_u128(x) < TWO_POW_6_RAW, "exp math exp is too large");
    let mut result = one();
    if (to_u128(x) == 0) {
        result = one();
    } else if (sign == 0) {
        result = div_fp(one(), exp(1, x));
    } else if (to_u128(x) == ONE_RAW) {
        result = from_u128(EXP_1_RAW);
    } else {
        result = one();

        if (to_u128(x) >= TWO_POW_5_RAW) {
            x = sub_fp(x, from_u128(TWO_POW_5_RAW));
            result = mul_fp(result, from_u128(EXP_32_RAW));
        };
        if (to_u128(x) >= TWO_POW_4_RAW) {
            x = sub_fp(x, from_u128(TWO_POW_4_RAW));
            result = mul_fp(result, from_u128(EXP_16_RAW));
        };
        if (to_u128(x) >= TWO_POW_3_RAW) {
            x = sub_fp(x, from_u128(TWO_POW_3_RAW));
            result = mul_fp(result, from_u128(EXP_8_RAW));
        };
        if (to_u128(x) >= TWO_POW_2_RAW) {
            x = sub_fp(x, from_u128(TWO_POW_2_RAW));
            result = mul_fp(result, from_u128(EXP_4_RAW));
        };
        if (to_u128(x) >= TWO_RAW) {
            x = sub_fp(x, from_u128(TWO_RAW));
            result = mul_fp(result, from_u128(EXP_2_RAW));
        };
        if (to_u128(x) >= ONE_RAW) {
            x = sub_fp(x, from_u128(ONE_RAW));
            result = mul_fp(result, from_u128(EXP_1_RAW));
        };
        if (to_u128(x) >= TWO_POW_NEG_1_RAW) {
            x = sub_fp(x, from_u128(TWO_POW_NEG_1_RAW));
            result = mul_fp(result, from_u128(EXP_1_OVER_2_RAW));
        };
        if (to_u128(x) >= TWO_POW_NEG_2_RAW) {
            x = sub_fp(x, from_u128(TWO_POW_NEG_2_RAW));
            result = mul_fp(result, from_u128(EXP_1_OVER_4_RAW));
        };
        if (to_u128(x) >= TWO_POW_NEG_3_RAW) {
            x = sub_fp(x, from_u128(TWO_POW_NEG_3_RAW));
            result = mul_fp(result, from_u128(EXP_1_OVER_8_RAW));
        };
        if (to_u128(x) >= TWO_POW_NEG_4_RAW) {
            x = sub_fp(x, from_u128(TWO_POW_NEG_4_RAW));
            result = mul_fp(result, from_u128(EXP_1_OVER_16_RAW));
        };

        // now, x is in the range [0, e^{1/16})
        // if x is 0, we can directly return
        // otherwise, use Taylor series expansion for e^x: 1 + x + (x^2 / 2!) + (x^3 / 3!) + ... + (x^n / n!).

        if (to_u128(x) != 0) {
            let mut term = x;
            let mut series_sum = one();
            series_sum = add_fp(series_sum, term);

            term = div(mul_fp(term, x), 2);
            series_sum = add_fp(series_sum, term);

            term = div(mul_fp(term, x), 3);
            series_sum = add_fp(series_sum, term);

            term = div(mul_fp(term, x), 4);
            series_sum = add_fp(series_sum, term);

            term = div(mul_fp(term, x), 5);
            series_sum = add_fp(series_sum, term);

            term = div(mul_fp(term, x), 6);
            series_sum = add_fp(series_sum, term);

            term = div(mul_fp(term, x), 7);
            series_sum = add_fp(series_sum, term);

            term = div(mul_fp(term, x), 8);
            series_sum = add_fp(series_sum, term);

            result = mul_fp(result, series_sum);
        };
    };
    result
}

//spec exp {
//    // opaque is required for recursive function
//    // otherwise move prover will complain even if we don't prove anything here
//    pragma opaque;
//}

pub fn pow(x: FixedPoint64, y: FixedPoint64) -> FixedPoint64 {
    let (success, result) = try_simple_pow(x, y);
    if (success) {
        result
    } else {
        pow_internal(x, y)
    }
}

/// pow_up multiplies pow result by (1 + 10^-9) if numerical approximation is used in pow
/// based on experiments, the result is always greater than or equal to the true value
pub fn pow_up(x: FixedPoint64, y: FixedPoint64) -> FixedPoint64 {
    let (success, result) = try_simple_pow(x, y);
    if (success) {
        result
    } else {
        mul_fp(pow_internal(x, y), from_u128(ONE_PLUS_TEN_EXP_MINUS_9))
    }
}

/// pow_down multiplies pow result by (1 - 10^-9) if numerical approximation is used in pow
/// based on experiments, the result is always smaller than or equal to the true value
pub fn pow_down(x: FixedPoint64, y: FixedPoint64) -> FixedPoint64 {
    let (success, result) = try_simple_pow(x, y);
    if (success) {
        result
    } else {
        mul_fp(pow_internal(x, y), from_u128(ONE_MINUS_TEN_EXP_MINUS_9))
    }
}

/// try_simple_pow returns the result of pow if it can be computed using simple rules
/// e.g. x^0 = 1, x^1 = x, x^2 = x * x, x^4 = x^2 * x^2
/// returns (true, value) if the result can be computed
/// returns (false, 0) if the result cannot be computed
fn try_simple_pow(x: FixedPoint64, y: FixedPoint64) -> (bool, FixedPoint64) {
    if (to_u128(y) == 0) {
        // We solve the 0^0 indetermination by making it equal to one.
        (true, one())
    } else if (to_u128(x) == 0) {
        (true, zero())
    } else if (to_u128(y) == ONE_RAW) {
        (true, x)
    } else if (to_u128(y) == TWO_RAW) {
        (true, mul_fp(x, x))
    } else if (to_u128(y) == TWO_POW_2_RAW) {
        let x_squared = mul_fp(x, x);
        (true, mul_fp(x_squared, x_squared))
    } else {
        (false, zero())
    }
}

// formula: x^y = exp(y * ln(x))
// this function does not check validity of x and y. Caller should ensure handling of special cases such as x = 0 or y = 0
fn pow_internal(x: FixedPoint64, y: FixedPoint64) -> FixedPoint64 {
    let (sign, ln_x) = ln(x);
    let y_times_ln_x = mul_fp(y, ln_x);
    exp(sign, y_times_ln_x)
}
