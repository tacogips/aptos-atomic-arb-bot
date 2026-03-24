/// Liquidswap liquidity pool module.
/// Implements mint/burn liquidity, swap of coins.
module liquidswap::liquidity_pool {
    use aptos_framework::coin::Coin;
    use aptos_framework::coin;
    use aptos_std::event;
    use liquidswap_lp::lp_coin::LP;
    use liquidswap::coin_helper;

    struct IsEmergency has key {}
    /// When coins used to create pair have wrong ordering.
    const ERR_WRONG_PAIR_ORDERING: u64 = 100;
    /// When pool doesn't exists for pair.
    const ERR_POOL_DOES_NOT_EXIST: u64 = 107;
    /// When pool is locked.
    const ERR_POOL_IS_LOCKED: u64 = 111;
    /// When both X and Y provided for flashloan are equal zero.
    const ERR_EMPTY_COIN_LOAN: u64 = 110;

    /// When both X and Y provided for swap are equal zero.
    const ERR_EMPTY_COIN_IN: u64 = 104;

    /// When attempted to execute operation during an emergency.
    const ERR_EMERGENCY: u64 = 4001;
    /// When emergency functional disabled.
    const ERR_DISABLED: u64 = 4002;
    /// Denominator to handle decimal points for fees.
    const FEE_SCALE: u64 = 10000;



    struct PoolCreatedEvent<phantom X, phantom Y, phantom Curve> has drop, store {
        creator: address,
    }

    struct LiquidityAddedEvent<phantom X, phantom Y, phantom Curve> has drop, store {
        added_x_val: u64,
        added_y_val: u64,
        lp_tokens_received: u64,
    }

    struct LiquidityRemovedEvent<phantom X, phantom Y, phantom Curve> has drop, store {
        returned_x_val: u64,
        returned_y_val: u64,
        lp_tokens_burned: u64,
    }

    struct SwapEvent<phantom X, phantom Y, phantom Curve> has drop, store {
        x_in: u64,
        x_out: u64,
        y_in: u64,
        y_out: u64,
    }

    struct FlashloanEvent<phantom X, phantom Y, phantom Curve> has drop, store {
        x_in: u64,
        x_out: u64,
        y_in: u64,
        y_out: u64,
    }

    struct OracleUpdatedEvent<phantom X, phantom Y, phantom Curve> has drop, store {
        last_price_x_cumulative: u128,
        last_price_y_cumulative: u128,
    }

    struct UpdateFeeEvent<phantom X, phantom Y, phantom Curve> has drop, store {
        new_fee: u64,
    }

    struct UpdateDAOFeeEvent<phantom X, phantom Y, phantom Curve> has drop, store {
        new_fee: u64,
    }


    // Events
    struct EventsStore<phantom X, phantom Y, phantom Curve> has key {
        pool_created_handle: event::EventHandle<PoolCreatedEvent<X, Y, Curve>>,
        liquidity_added_handle: event::EventHandle<LiquidityAddedEvent<X, Y, Curve>>,
        liquidity_removed_handle: event::EventHandle<LiquidityRemovedEvent<X, Y, Curve>>,
        swap_handle: event::EventHandle<SwapEvent<X, Y, Curve>>,
        flashloan_handle: event::EventHandle<FlashloanEvent<X, Y, Curve>>,
        oracle_updated_handle: event::EventHandle<OracleUpdatedEvent<X, Y, Curve>>,
        update_fee_handle: event::EventHandle<UpdateFeeEvent<X, Y, Curve>>,
        update_dao_fee_handle: event::EventHandle<UpdateDAOFeeEvent<X, Y, Curve>>,
    }



    /// Flash loan resource.
    /// There is no way in Move to pass calldata and make dynamic calls, but a resource can be used for this purpose.
    /// To make the execution into a single transaction, the flash loan function must return a resource
    /// that cannot be copied, cannot be saved, cannot be dropped, or cloned.
    struct Flashloan<phantom X, phantom Y, phantom Curve> {
        x_loan: u64,
        y_loan: u64
    }


    /// Get flash loan coins.
    /// In the most of situation only X or Y coin argument has value.
    /// Because an user usually loans only one coin, yet function allow to loans both coin.
    /// * `x_loan` - expected amount of X coins to loan.
    /// * `y_loan` - expected amount of Y coins to loan.
    /// Returns both loaned X and Y coins: `(Coin<X>, Coin<Y>, Flashloan<X, Y>)`.
    public fun flashloan<X, Y, Curve>(x_loan: u64, y_loan: u64): (Coin<X>, Coin<Y>, Flashloan<X, Y, Curve>)
    acquires LiquidityPool, EventsStore {
        assert_no_emergency();

        assert!(coin_helper::is_sorted<X, Y>(), ERR_WRONG_PAIR_ORDERING);
        assert!(exists<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account), ERR_POOL_DOES_NOT_EXIST);

        assert_pool_unlocked<X, Y, Curve>();

        assert!(x_loan > 0 || y_loan > 0, ERR_EMPTY_COIN_LOAN);

        let pool = borrow_global_mut<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account);

        let reserve_x = coin::value(&pool.coin_x_reserve);
        let reserve_y = coin::value(&pool.coin_y_reserve);

        // Withdraw expected amount from reserves.
        let x_loaned = coin::extract(&mut pool.coin_x_reserve, x_loan);
        let y_loaned = coin::extract(&mut pool.coin_y_reserve, y_loan);

        // The pool will be locked after the loan until payment.
        pool.locked = true;

        update_oracle(pool, reserve_x, reserve_y);

        // Return loaned amount.
        (x_loaned, y_loaned, Flashloan<X, Y, Curve> { x_loan, y_loan })
    }

    /// Update current cumulative prices.
    /// Important: If you want to use the following function take into account prices can be overflowed.
    /// So it's important to use same logic in your math/algo (as Move doesn't allow overflow). See math::overflow_add.
    /// * `pool` - Liquidity pool to update prices.
    /// * `x_reserve` - coin X reserves.
    /// * `y_reserve` - coin Y reserves.
    fun update_oracle<X, Y, Curve>(
        _pool: &mut LiquidityPool<X, Y, Curve>,
        _x_reserve: u64,
        _y_reserve: u64
    ) acquires EventsStore {
        //let last_block_timestamp = pool.last_block_timestamp;



            //let last_price_x_cumulative = uq64x64::to_u128(uq64x64::fraction(y_reserve, x_reserve)) * time_elapsed;
            //let last_price_y_cumulative = uq64x64::to_u128(uq64x64::fraction(x_reserve, y_reserve)) * time_elapsed;

            //pool.last_price_x_cumulative = math::overflow_add(pool.last_price_x_cumulative, last_price_x_cumulative);
            //pool.last_price_y_cumulative = math::overflow_add(pool.last_price_y_cumulative, last_price_y_cumulative);

            let events_store = borrow_global_mut<EventsStore<X, Y, Curve>>(@liquidswap_pool_account);
            event::emit_event(
                &mut events_store.oracle_updated_handle,
                OracleUpdatedEvent<X, Y, Curve> {
                    last_price_x_cumulative: 0,
                    last_price_y_cumulative: 0,
                });

    }


    /// Pay flash loan coins.
    /// In the most of situation only X or Y coin argument has value.
    /// Because an user usually loans only one coin, yet function allow to loans both coin.
    /// * `x_in` - X coins to pay.
    /// * `y_in` - Y coins to pay.
    /// * `loan` - data about flashloan.
    public fun pay_flashloan<X, Y, Curve>(
        x_in: Coin<X>,
        y_in: Coin<Y>,
        loan: Flashloan<X, Y, Curve>
    ) acquires LiquidityPool, EventsStore {
        assert_no_emergency();

        assert!(coin_helper::is_sorted<X, Y>(), ERR_WRONG_PAIR_ORDERING);
        assert!(exists<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account), ERR_POOL_DOES_NOT_EXIST);

        let Flashloan { x_loan, y_loan } = loan;

        let x_in_val = coin::value(&x_in);
        let y_in_val = coin::value(&y_in);

        assert!(x_in_val > 0 || y_in_val > 0, ERR_EMPTY_COIN_IN);

        let pool = borrow_global_mut<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account);

        let x_reserve_size = coin::value(&pool.coin_x_reserve);
        let y_reserve_size = coin::value(&pool.coin_y_reserve);

        // Reserve sizes before loan out
        x_reserve_size = x_reserve_size + x_loan;
        y_reserve_size = y_reserve_size + y_loan;

        // Deposit new coins to liquidity pool.
        coin::merge(&mut pool.coin_x_reserve, x_in);
        coin::merge(&mut pool.coin_y_reserve, y_in);

        // Confirm that lp_value for the pool hasn't been reduced.
        // For that, we compute lp_value with old reserves and lp_value with reserves after swap is done,
        // and make sure lp_value doesn't decrease
        let (x_res_new_after_fee, y_res_new_after_fee) =
            new_reserves_after_fees_scaled<Curve>(
                coin::value(&pool.coin_x_reserve),
                coin::value(&pool.coin_y_reserve),
                x_in_val,
                y_in_val,
                pool.fee,
            );
        assert_lp_value_is_increased<Curve>(
            pool.x_scale,
            pool.y_scale,
            (x_reserve_size as u128),
            (y_reserve_size as u128),
            x_res_new_after_fee,
            y_res_new_after_fee,
        );
        // third of all fees goes into DAO
        //split_fee_to_dao(pool, x_in_val, y_in_val);

        // As we are in same block, don't need to update oracle, it's already updated during flashloan initalization.

        // The pool will be unlocked after payment.
        pool.locked = false;

        let events_store = borrow_global_mut<EventsStore<X, Y, Curve>>(@liquidswap_pool_account);
        event::emit_event(
            &mut events_store.flashloan_handle,
            FlashloanEvent<X, Y, Curve> {
                x_in: x_in_val,
                x_out: x_loan,
                y_in: y_in_val,
                y_out: y_loan,
            });
    }


    /// Compute and verify LP value after and before swap, in nutshell, _k function.
    /// * `x_scale` - 10 pow by X coin decimals.
    /// * `y_scale` - 10 pow by Y coin decimals.
    /// * `x_res` - X reserves before swap.
    /// * `y_res` - Y reserves before swap.
    /// * `x_res_with_fees` - X reserves after swap.
    /// * `y_res_with_fees` - Y reserves after swap.
    /// Aborts if swap can't be done.
    fun assert_lp_value_is_increased<Curve>(
        _x_scale: u64,
        _y_scale: u64,
        _x_res: u128,
        _y_res: u128,
        _x_res_with_fees: u128,
        _y_res_with_fees: u128,
    ) {
		// do nothing
    }


    /// Get reserves after fees.
    /// * `x_reserve` - reserve X.
    /// * `y_reserve` - reserve Y.
    /// * `x_in_val` - amount of X coins added to reserves.
    /// * `y_in_val` - amount of Y coins added to reserves.
    /// * `fee` - amount of fee.
    /// Returns both X and Y reserves after fees.
    fun new_reserves_after_fees_scaled<Curve>(
        _x_reserve: u64,
        _y_reserve: u64,
        _x_in_val: u64,
        _y_in_val: u64,
        _fee: u64,
    ): (u128, u128) {
        (0,0)
    }




    /// Liquidity pool with reserves.
    struct LiquidityPool<phantom X, phantom Y, phantom Curve> has key {
        coin_x_reserve: Coin<X>,
        coin_y_reserve: Coin<Y>,
        last_block_timestamp: u64,
        last_price_x_cumulative: u128,
        last_price_y_cumulative: u128,
        lp_mint_cap: coin::MintCapability<LP<X, Y, Curve>>,
        lp_burn_cap: coin::BurnCapability<LP<X, Y, Curve>>,
        // Scales are pow(10, token_decimals).
        x_scale: u64,
        y_scale: u64,
        locked: bool,
        fee: u64,           // 1 - 100 (0.01% - 1%)
        dao_fee: u64,       // 0 - 100 (0% - 100%)
    }
    /// Get reserves of a pool.
    /// Returns both (X, Y) reserves.
    public fun get_reserves_size<X, Y, Curve>(): (u64, u64)
    acquires LiquidityPool {
        assert_no_emergency();

        assert!(coin_helper::is_sorted<X, Y>(), ERR_WRONG_PAIR_ORDERING);
        assert!(exists<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account), ERR_POOL_DOES_NOT_EXIST);

        assert_pool_unlocked<X, Y, Curve>();

        let liquidity_pool = borrow_global<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account);
        let x_reserve = coin::value(&liquidity_pool.coin_x_reserve);
        let y_reserve = coin::value(&liquidity_pool.coin_y_reserve);

        (x_reserve, y_reserve)
    }
    /// Get decimals scales (10^X decimals, 10^Y decimals) for stable curve.
    /// For uncorrelated curve would return just zeros.
    public fun get_decimals_scales<X, Y, Curve>(): (u64, u64) acquires LiquidityPool {
        assert!(coin_helper::is_sorted<X, Y>(), ERR_WRONG_PAIR_ORDERING);
        assert!(exists<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account), ERR_POOL_DOES_NOT_EXIST);

        let pool = borrow_global<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account);
        (pool.x_scale, pool.y_scale)
    }
    /// Get fee for specific pool together with denominator (numerator, denominator).
    public fun get_fees_config<X, Y, Curve>(): (u64, u64) acquires LiquidityPool {
        (get_fee<X, Y, Curve>(), FEE_SCALE)
    }
    /// Get fee for specific pool.
    public fun get_fee<X, Y, Curve>(): u64 acquires LiquidityPool {
        assert!(coin_helper::is_sorted<X, Y>(), ERR_WRONG_PAIR_ORDERING);
        assert!(exists<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account), ERR_POOL_DOES_NOT_EXIST);

        let pool = borrow_global<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account);
        pool.fee
    }
    /// Get if it's paused or not.
    public fun is_emergency(): bool {
        exists<IsEmergency>(@liquidswap_emergency_account)
    }
    /// Would abort if currently paused.
    public fun assert_no_emergency() {
        assert!(!is_emergency(), ERR_EMERGENCY);
    }
    /// Aborts if pool is locked.
    fun assert_pool_unlocked<X, Y, Curve>() acquires LiquidityPool {
        let pool = borrow_global<LiquidityPool<X, Y, Curve>>(@liquidswap_pool_account);
        assert!(pool.locked == false, ERR_POOL_IS_LOCKED);
    }
}
