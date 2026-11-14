pub fn calc_flashloan_fee(amount: u64) -> u64 {
    amount * 31 / 10_000
}
