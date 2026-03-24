
module liquidswap::router_v2{
	use aptos_framework::coin::{Self,Coin};

    /// Swap exact amount of coin `X` for coin `Y`.
    /// * `coin_in` - coin X to swap.
    /// * `coin_out_min_val` - minimum amount of coin Y to get out.
    /// Returns `Coin<Y>`.
    public fun swap_exact_coin_for_coin<X, Y, Curve>(
        coin_in: Coin<X>,
        _coin_out_min_val: u64,
    ): Coin<Y> {
        coin::destroy_zero(coin_in);
        coin::zero<Y>()
	}
}
