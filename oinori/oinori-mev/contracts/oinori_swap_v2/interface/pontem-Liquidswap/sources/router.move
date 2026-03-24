
module liquidswap::router {
    use aptos_framework::coin::{Self, Coin};
    use liquidswap::coin_helper;
    use liquidswap::liquidity_pool;
    use liquidswap::curves;
    use liquidswap::math;
    use liquidswap::stable_curve;
    /// Marks the unreachable place in code
    const ERR_UNREACHABLE: u64 = 207;

    public fun swap_exact_coin_for_coin<X, Y, LP>(
        coin_in: Coin<X>,
        _mint_out_amt: u64
    ): Coin<Y> {
        coin::destroy_zero(coin_in);
        coin::zero<Y>()
    }

    /// Swap max coin amount `X` for exact coin `Y`.
    /// * `coin_max_in` - maximum amount of coin X to swap to get `coin_out_val` of coins Y.
    /// * `coin_out_val` - exact amount of coin Y to get.
    /// Returns remainder of `coin_max_in` as `Coin<X>` and `Coin<Y>`: `(Coin<X>, Coin<Y>)`.
    public fun swap_coin_for_exact_coin<X, Y, Curve>(
        coin_max_in: Coin<X>,
        _coin_out_val: u64,
    ): (Coin<X>, Coin<Y>){
        (coin_max_in,coin::zero<Y>())
    }

    /// Get amount out for `amount_in` of X coins (see generic).
    /// So if Coins::USDC is X and Coins::USDT is Y, it will get amount of USDT you will get after swap `amount_x` USDC.
    /// !Important!: This function can eat a lot of gas if you querying it for stable curve pool, so be aware.
    /// We recommend to do implement such kind of logic offchain.
    /// * `amount_x` - amount to swap.
    /// Returns amount of `Y` coins getting after swap.
    public fun get_amount_out<X, Y, Curve>(amount_in: u64): u64 {
        let (reserve_x, reserve_y) = get_reserves_size<X, Y, Curve>();
        let (scale_x, scale_y) = get_decimals_scales<X, Y, Curve>();
        get_coin_out_with_fees<X, Y, Curve>(
            amount_in,
            reserve_x,
            reserve_y,
            scale_x,
            scale_y,
        )
    }

    /// Get amount in for `amount_out` of X coins (see generic).
    /// So if Coins::USDT is X and Coins::USDC is Y, you pass how much USDC you want to get and
    /// it returns amount of USDT you have to swap (include fees).
    /// !Important!: This function can eat a lot of gas if you querying it for stable curve pool, so be aware.
    /// We recommend to do implement such kind of logic offchain.
    /// * `amount_x` - amount to swap.
    /// Returns amount of `X` coins needed.
    public fun get_amount_in<X, Y, Curve>(amount_out: u64): u64 {
        let (reserve_x, reserve_y) = get_reserves_size<X, Y, Curve>();
        let (scale_x, scale_y) = get_decimals_scales<X, Y, Curve>();
        get_coin_in_with_fees<X, Y, Curve>(
            amount_out,
            reserve_y,
            reserve_x,
            scale_y,
            scale_x,
        )
    }

    /// Get reserves of liquidity pool (`X` and `Y`).
    /// Returns current reserves (`X`, `Y`).
    public fun get_reserves_size<X, Y, Curve>(): (u64, u64) {
        if (coin_helper::is_sorted<X, Y>()) {
            liquidity_pool::get_reserves_size<X, Y, Curve>()
        } else {
            let (y_res, x_res) = liquidity_pool::get_reserves_size<Y, X, Curve>();
            (x_res, y_res)
        }
    }
    /// Get decimals scales for stable curve, for uncorrelated curve would return zeros.
    /// Returns `X` and `Y` coins decimals scales.
    public fun get_decimals_scales<X, Y, Curve>(): (u64, u64) {
        if (coin_helper::is_sorted<X, Y>()) {
            liquidity_pool::get_decimals_scales<X, Y, Curve>()
        } else {
            let (y, x) = liquidity_pool::get_decimals_scales<Y, X, Curve>();
            (x, y)
        }
    }
    /// Get coin amount out by passing amount in (include fees). Pass all data manually.
    /// * `coin_in` - exactly amount of coins to swap.
    /// * `reserve_in` - reserves of coin we are going to swap.
    /// * `reserve_out` - reserves of coin we are going to get.
    /// * `scale_in` - 10 pow by decimals amount of coin we going to swap.
    /// * `scale_out` - 10 pow by decimals amount of coin we going to get.
    /// Returns amount of coins out after swap.
    fun get_coin_out_with_fees<X, Y, Curve>(
        coin_in: u64,
        reserve_in: u64,
        reserve_out: u64,
        scale_in: u64,
        scale_out: u64,
    ): u64 {
        let (fee_pct, fee_scale) = get_fees_config<X, Y, Curve>();
        let fee_multiplier = fee_scale - fee_pct;

        if (curves::is_stable<Curve>()) {
            let coin_in_val_scaled = math::mul_to_u128(coin_in, fee_multiplier);
            let coin_in_val_after_fees = if (coin_in_val_scaled % (fee_scale as u128) != 0) {
                (coin_in_val_scaled / (fee_scale as u128)) + 1
            } else {
                coin_in_val_scaled / (fee_scale as u128)
            };

            (stable_curve::coin_out(
                (coin_in_val_after_fees as u128),
                scale_in,
                scale_out,
                (reserve_in as u128),
                (reserve_out as u128)
            ) as u64)
        } else if (curves::is_uncorrelated<Curve>()) {
            let coin_in_val_after_fees = coin_in * fee_multiplier;
            let new_reserve_in = reserve_in * fee_scale + coin_in_val_after_fees;

            // Multiply coin_in by the current exchange rate:
            // current_exchange_rate = reserve_out / reserve_in
            // amount_in_after_fees * current_exchange_rate -> amount_out
            math::mul_div(coin_in_val_after_fees,
                reserve_out,
                new_reserve_in)
        } else {
            abort ERR_UNREACHABLE
        }
    }
    /// Get coin amount in by amount out. Pass all data manually.
    /// * `coin_out` - exactly amount of coins we want to get.
    /// * `reserve_out` - reserves of coin we are going to get.
    /// * `reserve_in` - reserves of coin we are going to swap.
    /// * `scale_in` - 10 pow by decimals amount of coin we swap.
    /// * `scale_out` - 10 pow by decimals amount of coin we get.
    ///
    /// This computation is a reverse of get_coin_out formula for uncorrelated assets:
    ///     y = x * (fee_scale - fee_pct) * ry / (rx + x * (fee_scale - fee_pct))
    ///
    /// solving it for x returns this formula:
    ///     x = y * rx / ((ry - y) * (fee_scale - fee_pct)) or
    ///     x = y * rx * (fee_scale) / ((ry - y) * (fee_scale - fee_pct)) which implemented in this function
    ///
    ///  For stable curve math described in `coin_in` func into `../libs/StableCurve.move`.
    ///
    /// Returns amount of coins needed for swap.
    fun get_coin_in_with_fees<X, Y, Curve>(
        coin_out: u64,
        reserve_out: u64,
        reserve_in: u64,
        scale_out: u64,
        scale_in: u64,
    ): u64 {
        let (fee_pct, fee_scale) = get_fees_config<X, Y, Curve>();
        let fee_multiplier = fee_scale - fee_pct;

        if (curves::is_stable<Curve>()) {
            // !!!FOR AUDITOR!!!
            // Double check it.
            let coin_in = (stable_curve::coin_in(
                (coin_out as u128),
                scale_out,
                scale_in,
                (reserve_out as u128),
                (reserve_in as u128),
            ) as u64) + 1;

            (coin_in * fee_scale / fee_multiplier) + 1
        } else if (curves::is_uncorrelated<Curve>()) {
            let new_reserves_out = (reserve_out - coin_out) * fee_multiplier;

            // coin_out * reserve_in * fee_scale / new reserves out
            let coin_in = math::mul_div(
                coin_out, // y
                reserve_in * fee_scale,
                new_reserves_out
            ) + 1;
            coin_in
        } else {
            abort ERR_UNREACHABLE
        }
    }
    /// Get fee for specific pool together with denominator (numerator, denominator).
    public fun get_fees_config<X, Y, Curve>(): (u64, u64) {
        if (coin_helper::is_sorted<X, Y>()) {
            liquidity_pool::get_fees_config<X, Y, Curve>()
        } else {
            liquidity_pool::get_fees_config<Y, X, Curve>()
        }
    }
}
