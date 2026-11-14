script {

	use std::signer;
    use std::option;
    use aptos_framework::coin;
	use liquidswap::liquidity_pool;
	use hippo_aggregator::aggregator;

	// only AMM AMM
	const POOL_TYPE :u64 = 0;

    const E_NOT_SUFFICIENT: u64 = 10;
    const E_NOT_SUFFICIENT_2: u64 = 20;

	fun main<
		Y,Z, C1, C2, C3, BY, BC
		>(signer:&signer,
		coin_amount_in:u64,
		min_out_amount:u64,

		num_steps:u8,

		first_dex_type:u8,
		first_is_x_to_y:bool,

		second_dex_type:u8,
		second_is_x_to_y:bool,

		third_dex_type:u8,
		third_is_x_to_y:bool){

		// --- borrow  ------------------
		let (coin_borrowd_zero, coin_borrowed, loan) = liquidity_pool::flashloan<BY, 0x1::aptos_coin::AptosCoin, BC>(0, coin_amount_in);

		let signer_address = signer::address_of(signer);

		let borrowed_amount = coin::value(&coin_borrowed);
		coin::deposit(signer_address, coin_borrowed);


		// ---- swap -----------------


		//    public fun swap_direct<
		//        X, Y, Z, OutCoin, E1, E2, E3
		//    >(
		//        num_steps: u8,
		//        first_dex_type: u8,
		//        first_pool_type: u64,
		//        first_is_x_to_y: bool, // first trade uses normal order
		//        second_dex_type: u8,
		//        second_pool_type: u64,
		//        second_is_x_to_y: bool, // second trade uses normal order
		//        third_dex_type: u8,
		//        third_pool_type: u64,
		//        third_is_x_to_y: bool, // second trade uses normal order
		//        x_in: coin::Coin<X>
		//    ):(Option<coin::Coin<X>>, Option<coin::Coin<Y>>, Option<coin::Coin<Z>>, coin::Coin<OutCoin>) acquires EventStore, CoinStore, TortugaSigner {

		////#[cmd]
		////public entry fun swap<
		////    X, Y, Z, OutCoin, E1, E2, E3
		////>(
		////    sender: &signer,
		////    num_steps: u8,
		////    first_dex_type: u8,
		////    first_pool_type: u64,
		////    first_is_x_to_y: bool, // first trade uses normal order
		////    second_dex_type: u8,
		////    second_pool_type: u64,
		////    second_is_x_to_y: bool, // second trade uses normal order
		////    third_dex_type: u8,
		////    third_pool_type: u64,
		////    third_is_x_to_y: bool, // second trade uses normal order
		////    x_in: u64,
		////    m_min_out: u64,
		////) acquires EventStore, CoinStore, TortugaSigner {

		//aggregator::swap<
		//	0x1::aptos_coin::AptosCoin, //X
		//	Y,
		//	Z,
		//	0x1::aptos_coin::AptosCoin , // OutCoin
		//	C1,C2,C3>(signer,
		//		num_steps,
		//		first_dex_type,
		//		POOL_TYPE, //first_pool_type: u64,
		//		first_is_x_to_y, // first trade uses normal order
		//		second_dex_type,
		//		POOL_TYPE , //second_pool_type: u64,
		//		second_is_x_to_y, // second trade uses normal order
		//		third_dex_type,
		//		POOL_TYPE, //third_pool_type: u64,
		//		third_is_x_to_y, // second trade uses normal order
		//		borrowed_amount , // x_in
		//		min_out_amount  //m_min_out
		//		);

        let coin_x = coin::withdraw(signer, borrowed_amount);
		let (x_remain, y_remain, z_remain, coin_m) = aggregator::swap_direct<
			0x1::aptos_coin::AptosCoin, //X
			Y,
			Z,
			0x1::aptos_coin::AptosCoin , // OutCoin
			C1,C2,C3>(
				num_steps,
				first_dex_type,
				POOL_TYPE, //first_pool_type: u64,
				first_is_x_to_y, // first trade uses normal order
				second_dex_type,
				POOL_TYPE , //second_pool_type: u64,
				second_is_x_to_y, // second trade uses normal order
				third_dex_type,
				POOL_TYPE, //third_pool_type: u64,
				third_is_x_to_y, // second trade uses normal order
				coin_x  // x_in
				);


        assert!(coin::value(&coin_m) >= min_out_amount, E_NOT_SUFFICIENT);

		// --- calc loan fee------------------
		let loan_fee_amount = borrowed_amount * 31 /10000;
		let repay_amount =  borrowed_amount + loan_fee_amount;

		if (min_out_amount > 0) {
			assert!(coin::value(&coin_m) >= repay_amount, E_NOT_SUFFICIENT_2);
		};

		// --- deposit ------------------
        //check_and_deposit_opt(sender, x_remain);
        //check_and_deposit_opt(sender, y_remain);
        //check_and_deposit_opt(sender, z_remain);
        //check_and_deposit(sender, coin_m);


        if (option::is_some(&x_remain)) {
            let coin = option::extract(&mut x_remain);
            let signer_addr = signer::address_of(signer);
            coin::deposit(signer_addr, coin);
        };
        option::destroy_none(x_remain);

        if (option::is_some(&y_remain)) {
            let coin = option::extract(&mut y_remain);
            let signer_addr = signer::address_of(signer);
            if (!coin::is_account_registered<Y>(signer_addr)) {
                coin::register<Y>(signer);
            };
            coin::deposit(signer_addr, coin);
        };
        option::destroy_none(y_remain);


        if (option::is_some(&z_remain)) {
            let coin = option::extract(&mut z_remain);
            let signer_addr = signer::address_of(signer);
            if (!coin::is_account_registered<Z>(signer_addr)) {
                coin::register<Z>(signer);
            };
            coin::deposit(signer_addr, coin);
        };
        option::destroy_none(z_remain);

		// -- deposit
		coin::deposit(signer_address, coin_m);

		// --- repay the loan------------------
		let repay_coin = coin::withdraw<0x1::aptos_coin::AptosCoin>(signer, repay_amount);

        liquidity_pool::pay_flashloan(coin_borrowd_zero, repay_coin, loan);
	}


    //fun check_and_deposit_opt<X>(sender: &signer, coin_opt: Option<coin::Coin<X>>) {
    //    if (option::is_some(&coin_opt)) {
    //        let coin = option::extract(&mut coin_opt);
    //        let sender_addr = signer::address_of(sender);
    //        if (!coin::is_account_registered<X>(sender_addr)) {
    //            coin::register<X>(sender);
    //        };
    //        coin::deposit(sender_addr, coin);
    //    };
    //    option::destroy_none(coin_opt)
    //}

    //fun check_and_deposit<X>(sender: &signer, coin: coin::Coin<X>) {
    //    let sender_addr = signer::address_of(sender);
    //    if (!coin::is_account_registered<X>(sender_addr)) {
    //        coin::register<X>(sender);
    //    };
    //    coin::deposit(sender_addr, coin);
    //}



}
