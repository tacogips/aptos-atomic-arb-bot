module liquidswap::scripts {
    public entry fun register_pool_and_add_liquidity<X, Y, LP>(
        _creator: &signer,
        // _pool_type: u8, // 1 - stable; 2 - uncorrelated
        _x_amount: u64,
        _x_min_amount: u64,
        _y_amount: u64,
        _y_min_amount: u64,
    ) {

    }
}
