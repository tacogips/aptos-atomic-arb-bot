const APT_UNIT: f64 = 1_0000_0000f64;

pub fn apt_amount(amount: f64) -> u64 {
    (amount * APT_UNIT) as u64
}

pub fn apt_octa_amount(amount: u64) -> u64 {
    amount
}

pub fn in_apt_unit(amount: u64) -> f64 {
    amount as f64 / APT_UNIT as f64
}

pub fn in_signed_apt_unit(amount: i64) -> f64 {
    amount as f64 / APT_UNIT as f64
}
