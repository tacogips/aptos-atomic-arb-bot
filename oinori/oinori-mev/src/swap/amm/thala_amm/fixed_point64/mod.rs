pub mod log_exp_math;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

const TWO_POW_64: u128 = 1 << 64;
const TWO_POW_32: u128 = 1 << 32;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct FixedPoint64 {
    #[serde(deserialize_with = "from_u128_string")]
    #[serde(serialize_with = "serialize_as_str")]
    pub v: u128,
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

impl FixedPoint64 {
    /// Convert from `u128` (raw value) to FixedPoint64
    pub fn from_u128(v: u128) -> FixedPoint64 {
        from_u128(v)
    }
}

/// Encode `u64` to `FixedPoint64`
pub fn encode(x: u64) -> FixedPoint64 {
    let v = (x as u128) << 64;
    FixedPoint64 { v }
}

/// Decode a `FixedPoint64` into a `u64` by rounding to nearest integer
/// This should be the default way to convert back to integer
/// Unless you have a good reason to round up or down
pub fn decode(fp: FixedPoint64) -> u64 {
    let a = ((fp.v >> 64) as u64);
    let v_shifted = fp.v >> 63;
    // equivalent to (we want to avoid using `&`)
    // if (fp.v & mask > 0) a = a + 1;
    if (v_shifted % 2 == 1) {
        a + 1
    } else {
        a
    }
}

/// Decode a `FixedPoint64` into a `u64` by rounding down
pub fn decode_round_down(fp: FixedPoint64) -> u64 {
    let a = ((fp.v >> 64) as u64);
    a
}

/// Decode a `FixedPoint64` into a `u64` by rounding up
pub fn decode_round_up(fp: FixedPoint64) -> u64 {
    let a = ((fp.v >> 64) as u64);
    if (fp.v - (fp.v / TWO_POW_64) * TWO_POW_64 > 0) {
        a + 1
    } else {
        a
    }
}

/// Get `u128` (raw value) from FixedPoint64
pub fn to_u128(fp: FixedPoint64) -> u128 {
    fp.v
}

/// Convert from `u128` (raw value) to FixedPoint64
pub fn from_u128(v: u128) -> FixedPoint64 {
    FixedPoint64 { v }
}

/// Get integer "one" in FixedPoint64
pub fn one() -> FixedPoint64 {
    FixedPoint64 { v: TWO_POW_64 }
}

/// Get integer "zero" in FixedPoint64
pub fn zero() -> FixedPoint64 {
    FixedPoint64 { v: 0 }
}

/// Multiply a `FixedPoint64` by a `u64`, returning a `FixedPoint64`
pub fn mul(fp: FixedPoint64, y: u64) -> FixedPoint64 {
    // vm would direct abort when overflow occured
    let v = fp.v * (y as u128);

    FixedPoint64 { v }
}

/// Divide a `FixedPoint64` by a `u64`, returning a `FixedPoint64`.
pub fn div(fp: FixedPoint64, y: u64) -> FixedPoint64 {
    debug_assert!(y != 0, "fixed point divided by zero");

    let v = fp.v / (y as u128);
    FixedPoint64 { v }
}

/// Add a `FixedPoint64` and a `u64`, returning a `FixedPoint64`
pub fn add(fp: FixedPoint64, y: u64) -> FixedPoint64 {
    // vm would direct abort when overflow occured
    let v = fp.v + ((y as u128) << 64);

    FixedPoint64 { v }
}

/// Subtract `FixedPoint64` by a `u64`, returning a `FixedPoint64`
pub fn sub(fp: FixedPoint64, y: u64) -> FixedPoint64 {
    // vm would direct abort when underflow occured
    let v = fp.v - ((y as u128) << 64);

    FixedPoint64 { v }
}

/// Add a `FixedPoint64` and a `FixedPoint64`, returning a `FixedPoint64`
pub fn add_fp(a: FixedPoint64, b: FixedPoint64) -> FixedPoint64 {
    // vm would direct abort when overflow occured
    let v = a.v + b.v;

    FixedPoint64 { v }
}

/// Subtract `FixedPoint64` by a `FixedPoint64`, returning a `FixedPoint64`
pub fn sub_fp(a: FixedPoint64, b: FixedPoint64) -> FixedPoint64 {
    // avoid underflow
    // vm would direct abort when underflow occured
    if a.v < b.v {
        FixedPoint64 { v: 0 }
    } else {
        let v = a.v - b.v;
        FixedPoint64 { v }
    }
}

/// Multiply a `FixedPoint64` by a `FixedPoint64`, returning a `FixedPoint64`
/// To avoid overflow, the result must be smaller than MAX_U64
pub fn mul_fp(a: FixedPoint64, b: FixedPoint64) -> FixedPoint64 {
    let a_shift = a.v >> 32;
    let b_shift = b.v >> 32;
    let a_low_32 = a.v - a_shift * TWO_POW_32;
    let b_low_32 = b.v - b_shift * TWO_POW_32;
    // vm would direct abort when overflow occured
    let v = a_shift * b_shift + ((a_shift * b_low_32) >> 32) + ((b_shift * a_low_32) >> 32);

    FixedPoint64 { v }
}

/// Divide a `FixedPoint64` by a `FixedPoint64`, returning a `FixedPoint64`.
/// To avoid overflow, the result must be smaller than MAX_U64
pub fn div_fp(a: FixedPoint64, b: FixedPoint64) -> FixedPoint64 {
    let b_shift = b.v >> 32;
    debug_assert!(b_shift != 0, "fixed fp div_fp diviser too small");

    let result = a.v / b_shift;
    // make sure result << 32 won't overflow
    debug_assert!(result >> 96 == 0, "fixed fp div_fp diviser too big");
    let v = result << 32;

    FixedPoint64 { v }
}

/// Returns a `FixedPoint64` which represents the ratio of the numerator to the denominator.
pub fn fraction(numerator: u64, denominator: u64) -> FixedPoint64 {
    assert!(denominator != 0, "fixed point f64 fraction divided by zero");

    let r = (numerator as u128) << 64;
    let v = r / (denominator as u128);

    FixedPoint64 { v }
}

/// Less than
pub fn lt(left: &FixedPoint64, right: &FixedPoint64) -> bool {
    left < right
}

/// Greater than
pub fn gt(left: &FixedPoint64, right: &FixedPoint64) -> bool {
    left > right
}

/// Less or equal than
pub fn lte(left: &FixedPoint64, right: &FixedPoint64) -> bool {
    left <= right
}

/// Greater or equal than
pub fn gte(left: &FixedPoint64, right: &FixedPoint64) -> bool {
    left >= right
}

/// Equal than
pub fn eq(left: &FixedPoint64, right: &FixedPoint64) -> bool {
    left == right
}

/// Check if `FixedPoint64` is zero
pub fn is_zero(fp: &FixedPoint64) -> bool {
    fp.v == 0
}