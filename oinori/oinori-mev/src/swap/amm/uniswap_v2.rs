fn compute_amount(dx: u64, x: u64, y: u64) -> u64 {
    let numerator: u128 = y as u128 * dx as u128;
    let denominator = (x + dx) as u128;
    let dy = numerator / denominator;

    return dy as u64;
}

pub fn get_uniswap_v2_coin_out_with_fee(
    coin_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    fee_rate: u64,
    fee_scale: u64,
) -> u64 {
    let fee_coin = (coin_in as u128 * fee_rate as u128) / fee_scale as u128;
    let coin_in = coin_in - fee_coin as u64;

    let output_amount = compute_amount(coin_in, reserve_in, reserve_out);

    return output_amount;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_unswap_v2_calc_1() {
        let calced_amount = get_uniswap_v2_coin_out_with_fee(1000, 10000, 200000, 0, 10000);
        assert_eq!(calced_amount, 18181);
    }

    #[test]
    fn test_unswap_v2_calc_2() {
        let calced_amount =
            get_uniswap_v2_coin_out_with_fee(1500000000, 4680000000000, 18000000000, 30, 10000);
        assert_eq!(calced_amount, 5750085);
    }

    #[test]
    fn test_unswap_v2_calc_3() {
        let calced_amount =
            get_uniswap_v2_coin_out_with_fee(100000000, 18000000000, 4680000000000, 30, 10000);
        assert_eq!(calced_amount, 25779211810);
    }

    #[test]
    fn test_unswap_v2_calc_4() {
        let calced_amount =
            get_uniswap_v2_coin_out_with_fee(1000_000_000, 383146658729, 15805702755, 2, 1000);
        assert_eq!(calced_amount, 41062893);
    }

    #[test]
    fn test_aptoswa_calc_aptoswap_bug() {
        let calced_amount =
            get_uniswap_v2_coin_out_with_fee(71111954, 2386608352, 5566888308881, 30, 10_000);

        assert_eq!(calced_amount, 160603692231);
    }

    #[test]
    fn test_unswap_v2_calc_5_aux_bug() {
        let calced_amount =
            get_uniswap_v2_coin_out_with_fee(300000000, 658570468402, 6028693154990, 10, 10000);
        assert_eq!(calced_amount, 2742269152);
    }
}
