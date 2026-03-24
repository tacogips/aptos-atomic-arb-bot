const PRECISION_FACTOR: u128 = 1_000_000u128;

pub enum InputAmount {
    NotNormalized(UnPrecisedInputAmount),
    Normalized(UnPrecisedInputAmount),
}
impl InputAmount {
    pub fn new_not_normalized(v: u128) -> Self {
        Self::NotNormalized(UnPrecisedInputAmount(v))
    }

    pub fn new_normalized(v: u128) -> Self {
        Self::Normalized(UnPrecisedInputAmount(v))
    }

    pub fn update<F>(&self, f: F) -> Self
    where
        F: Fn(u128) -> u128,
    {
        match &self {
            Self::Normalized(v) => Self::new_normalized(f(v.0)),
            Self::NotNormalized(v) => Self::new_not_normalized(f(v.0)),
        }
    }

    pub fn normalize_if_need(&self, norm: &Normalizer) -> UnPrecisedInputAmount {
        match self {
            Self::NotNormalized(v) => UnPrecisedInputAmount(norm.normalize(v.0)),
            Self::Normalized(v) => *v,
        }
    }
}

#[derive(Clone, Copy)]
pub struct UnPrecisedInputAmount(u128);
impl UnPrecisedInputAmount {
    pub fn precise(self) -> PrecisedInputAmount {
        PrecisedInputAmount::from_unprecised(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedInputAmount(u128);
impl PrecisedInputAmount {
    pub fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR)
    }
    pub fn precised_amount(&self) -> PrecisedAmount {
        PrecisedAmount(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct UnPrecisedReserveIn(u128);
impl UnPrecisedReserveIn {
    pub fn new(v: u128) -> Self {
        Self(v)
    }

    pub fn normalize(self, normalizer: &Normalizer) -> UnPrecisedReserveIn {
        Self(normalizer.normalize(self.0))
    }
    pub fn prcise(&self) -> PrecisedReserveIn {
        PrecisedReserveIn::from_unprecised(self.0)
    }
    pub fn max_amount_with(&self, out: &UnPrecisedReserveOut) -> u128 {
        std::cmp::max(self.0, out.0)
    }

    pub fn as_reserve_out(&self) -> UnPrecisedReserveOut {
        UnPrecisedReserveOut::new(self.0)
    }

    pub fn un_precised_amount(&self) -> UnPrecisedAmount {
        UnPrecisedAmount(self.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnPrecisedAmount(u128);
impl UnPrecisedAmount {
    pub fn pow(&self, v: u32) -> Self {
        Self(self.0.pow(v))
    }

    pub fn sqrt(&self) -> Self {
        Self(sqrt(self.0))
    }

    pub fn normalize(self, normalizer: &Normalizer) -> Self {
        Self(normalizer.normalize(self.0))
    }
    pub fn amount(&self) -> u128 {
        self.0
    }
}

impl std::ops::Add<Self> for UnPrecisedAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub<Self> for UnPrecisedAmount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul<Self> for UnPrecisedAmount {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl std::ops::Mul<u128> for UnPrecisedAmount {
    type Output = Self;

    fn mul(self, rhs: u128) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl std::ops::Div<Self> for UnPrecisedAmount {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedAmount(u128);

impl PrecisedAmount {
    pub fn normalize(self, normalizer: &Normalizer) -> Self {
        Self(normalizer.normalize(self.0))
    }
    pub fn sub_m(&self, precised_m: PrecisedM) -> PrecisedAmount {
        PrecisedAmount(self.0 - precised_m.0)
    }
}

impl std::ops::Mul<Self> for PrecisedAmount {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

#[derive(Clone, Copy)]
pub struct UnPrecisedReserveOut(u128);
impl UnPrecisedReserveOut {
    pub fn new(v: u128) -> Self {
        Self(v)
    }
    pub fn normalize(self, normalizer: &Normalizer) -> UnPrecisedReserveOut {
        Self(normalizer.normalize(self.0))
    }
    pub fn prcise(&self) -> PrecisedReserveOut {
        PrecisedReserveOut::from_unprecised(self.0)
    }

    pub fn as_reserve_in(&self) -> UnPrecisedReserveIn {
        UnPrecisedReserveIn::new(self.0)
    }

    pub fn un_precised_amount(&self) -> UnPrecisedAmount {
        UnPrecisedAmount(self.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrecisedReserveIn(u128);
impl PrecisedReserveIn {
    pub fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR)
    }
    pub fn is_greater_than_xa(&self, xa: &PrecisedXa) -> bool {
        self.0 > xa.0
    }

    pub fn add_input_amount(&self, input_amount: PrecisedInputAmount) -> Self {
        Self(self.0 + input_amount.0)
    }

    pub fn add_m(&self, m: PrecisedM) -> Self {
        Self(self.0 + m.0)
    }

    pub fn sub_xa(&self, xa: PrecisedXa) -> PrecisedValueDelta {
        PrecisedValueDelta(self.0 - xa.0)
    }

    pub fn sub_n(&self, precised_n: PrecisedN) -> PrecisedAmount {
        PrecisedAmount(self.0 - precised_n.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrecisedReserveOut(u128);
impl PrecisedReserveOut {
    pub fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR)
    }

    pub fn sub_xb(&self, xb: PrecisedXb) -> PrecisedValueDelta {
        PrecisedValueDelta(self.0 - xb.0)
    }
    pub fn sub_or_zero(&self, other: &PrecisedAmount) -> PrecisedValueDelta {
        if self.0 > other.0 {
            PrecisedValueDelta(self.0 - other.0)
        } else {
            PrecisedValueDelta::zero()
        }
    }
}

#[derive(Clone, Copy)]
pub struct UnPrecisedXa(u128);
impl UnPrecisedXa {
    pub fn new(v: u128) -> Self {
        Self(v)
    }
    pub fn as_xb(&self) -> UnPrecisedXb {
        UnPrecisedXb::new(self.0)
    }

    pub fn convert_to_reserve_in(&self) -> UnPrecisedReserveIn {
        UnPrecisedReserveIn::new(self.0)
    }

    pub fn precise(self) -> PrecisedXa {
        PrecisedXa::from_unprecised(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedXa(u128);
impl PrecisedXa {
    pub fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR)
    }
}

#[derive(Clone, Copy)]
pub struct UnPrecisedXb(u128);
impl UnPrecisedXb {
    pub fn new(v: u128) -> Self {
        Self(v)
    }

    pub fn as_xa(&self) -> UnPrecisedXa {
        UnPrecisedXa::new(self.0)
    }

    pub fn convert_to_reserve_out(&self) -> UnPrecisedReserveOut {
        UnPrecisedReserveOut::new(self.0)
    }

    pub fn precise(self) -> PrecisedXb {
        PrecisedXb::from_unprecised(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedXb(u128);
impl PrecisedXb {
    pub fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR)
    }
}

#[derive(Clone, Copy)]
pub struct UnPrecisedM(u128);
impl UnPrecisedM {
    pub fn new(v: u128) -> Self {
        Self(v)
    }

    pub fn precise(self) -> PrecisedM {
        PrecisedM::from_unprecised(self.0)
    }

    pub fn un_precised_amount(&self) -> UnPrecisedAmount {
        UnPrecisedAmount(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedM(u128);
impl PrecisedM {
    pub fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrecisedN(u128);
impl PrecisedN {
    fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnPrecisedN(u128);
impl UnPrecisedN {
    pub fn new(v: u128) -> Self {
        Self(v)
    }

    pub fn precise(self) -> PrecisedN {
        PrecisedN::from_unprecised(self.0)
    }

    pub fn un_precised_amount(&self) -> UnPrecisedAmount {
        UnPrecisedAmount(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct UnPrecisedK(u128);
impl UnPrecisedK {
    pub fn new(v: u128) -> Self {
        Self(v)
    }

    pub fn precise(self) -> PrecisedK {
        PrecisedK::from_unprecised(self.0)
    }

    pub fn un_precised_amount(&self) -> UnPrecisedAmount {
        UnPrecisedAmount(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedK(u128);
impl PrecisedK {
    pub fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR * PRECISION_FACTOR)
    }
    pub fn div(&self, reserve_in: PrecisedReserveIn) -> PrecisedAmount {
        PrecisedAmount(self.0 / reserve_in.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnPrecisedK2(u128);
impl UnPrecisedK2 {
    pub fn new(v: u128) -> Self {
        Self(v)
    }

    pub fn precise(self) -> PrecisedK2 {
        PrecisedK2::from_unprecised(self.0)
    }

    pub fn un_precised_amount(&self) -> UnPrecisedAmount {
        UnPrecisedAmount(self.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrecisedK2(u128);
impl PrecisedK2 {
    pub fn from_unprecised(v: u128) -> Self {
        Self(v * PRECISION_FACTOR * PRECISION_FACTOR)
    }

    pub fn div(&self, prcied_amount: PrecisedAmount) -> PrecisedAmount {
        PrecisedAmount(self.0 / prcied_amount.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedKRatioByReserveIn(u128);
impl PrecisedKRatioByReserveIn {
    pub fn sub_m(&self, precised_m: PrecisedM) -> PrecisedReserveOut {
        PrecisedReserveOut(self.0 - precised_m.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedValueDelta(u128);
impl PrecisedValueDelta {
    pub fn zero() -> PrecisedValueDelta {
        PrecisedValueDelta(0)
    }
    pub fn un_presiced(&self) -> UnPrecisedValueDelta {
        UnPrecisedValueDelta(self.0 / PRECISION_FACTOR)
    }
    pub fn un_normalize(&self, normalizer: &Normalizer) -> PrecisedAmount {
        PrecisedAmount(normalizer.un_normalize(self.0))
    }
}

#[derive(Clone, Copy)]
pub struct UnPrecisedValueDelta(u128);

impl UnPrecisedValueDelta {
    pub fn as_normalized_input_amount(self) -> InputAmount {
        InputAmount::new_normalized(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrecisedNormalizedOutputAmount(u128);
impl PrecisedNormalizedOutputAmount {
    pub fn new(v: PrecisedAmount) -> Self {
        Self(v.0)
    }

    pub fn add_delta(&self, delta: &PrecisedValueDelta) -> PrecisedNormalizedOutputAmount {
        PrecisedNormalizedOutputAmount(self.0 + delta.0)
    }

    pub fn min(&self, other: &PrecisedNormalizedOutputAmount) -> PrecisedNormalizedOutputAmount {
        if self.0 > other.0 {
            *other
        } else {
            *self
        }
    }

    pub fn un_normalize(&self, normalizer: &Normalizer) -> Self {
        PrecisedNormalizedOutputAmount(normalizer.un_normalize(self.0))
    }

    pub fn un_precise(&self) -> u128 {
        self.0 / PRECISION_FACTOR
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Ratio {
    numerator: u128,
    denominator: u128,
}

impl Ratio {
    pub fn with_xa_xb(xa: UnPrecisedXa, xb: UnPrecisedXb) -> Self {
        Self {
            numerator: xa.0,
            denominator: xb.0,
        }
    }

    pub fn with_xb_xa(xb: UnPrecisedXb, xa: UnPrecisedXa) -> Self {
        Self {
            numerator: xb.0,
            denominator: xa.0,
        }
    }

    pub fn with_unprecised_reserves(cin: UnPrecisedReserveIn, cout: UnPrecisedReserveOut) -> Self {
        Self {
            numerator: cin.0,
            denominator: cout.0,
        }
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let x = self.numerator * other.denominator;
        let y = other.numerator * self.denominator;

        if x < y {
            Some(std::cmp::Ordering::Less)
        } else if x > y {
            Some(std::cmp::Ordering::Greater)
        } else {
            Some(std::cmp::Ordering::Equal)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Normalizer {
    numerator: u128,
    denominator: u128,
}
impl Default for Normalizer {
    fn default() -> Self {
        Self {
            numerator: 1,
            denominator: 1,
        }
    }
}

impl Normalizer {
    pub fn new(numerator: u128, denominator: u128) -> Self {
        Self {
            numerator,
            denominator,
        }
    }
    pub fn normalize(&self, value: u128) -> u128 {
        value * self.numerator / self.denominator
    }

    pub fn un_normalize(&self, value: u128) -> u128 {
        value * self.denominator / self.numerator
    }
}

fn sqrt(y: u128) -> u128 {
    if y < 4 {
        if y == 0 {
            0
        } else {
            1
        }
    } else {
        let mut z = y;
        let mut x = y / 2 + 1;
        while x < z {
            {
                z = x;
                x = (y / x + x) / 2;
            }
        }
        z
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_obric_sqrt() {
        assert_eq!(sqrt(4), 2u128);
        assert_eq!(sqrt(9), 3u128);
        assert_eq!(sqrt(16), 4u128);
        assert_eq!(sqrt(2), 1);
        assert_eq!(sqrt(3), 1);
        assert_eq!(sqrt(6), 2);
    }
}
