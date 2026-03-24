script {

	use hippo_aggregator::aggregator;
	use std::signer;
    use std::option;
    use aptos_framework::coin;

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
		first_pool_type:u64,

		second_dex_type:u8,
		second_is_x_to_y:bool,
		second_pool_type:u64,

		third_dex_type:u8,
		third_is_x_to_y:bool,
		third_pool_type:u64){

		// ---- swap -----------------

		//#[cmd]
		//public entry fun swap<
		//    X, Y, Z, OutCoin, E1, E2, E3
		//>(
		//    sender: &signer,
		//    num_steps: u8,
		//    first_dex_type: u8,
		//    first_pool_type: u64,
		//    first_is_x_to_y: bool, // first trade uses normal order
		//    second_dex_type: u8,
		//    second_pool_type: u64,
		//    second_is_x_to_y: bool, // second trade uses normal order
		//    third_dex_type: u8,
		//    third_pool_type: u64,
		//    third_is_x_to_y: bool, // second trade uses normal order
		//    x_in: u64,
		//    m_min_out: u64,
		//) acquires EventStore, CoinStore, TortugaSigner {

		// hippo might have a bug
		//aggregator::swap<
		//	0x1::aptos_coin::AptosCoin, //X
		//	Y,
		//	Z,
		//	0x1::aptos_coin::AptosCoin , // OutCoin
		//	C1,C2,C3>(signer,
		//		num_steps,
		//		first_dex_type,
		//		first_pool_type, //first_pool_type: u64,
		//		first_is_x_to_y, // first trade uses normal order
		//		second_dex_type,
		//		second_pool_type, //second_pool_type: u64,
		//		second_is_x_to_y, // second trade uses normal order
		//		third_dex_type,
		//		third_pool_type, //third_pool_type: u64,
		//		third_is_x_to_y, // second trade uses normal order
		//		coin_amount_in, // x_in
		//		min_out_amount  //m_min_out
		//		);

		let signer_address = signer::address_of(signer);
        let coin_x = coin::withdraw(signer, coin_amount_in);
		let (x_remain, y_remain, z_remain, coin_m) = aggregator::swap_direct<
			0x1::aptos_coin::AptosCoin, //X
			Y,
			Z,
			0x1::aptos_coin::AptosCoin , // OutCoin
			C1,C2,C3>(
				num_steps,
				first_dex_type,
				first_pool_type, //first_pool_type: u64,
				first_is_x_to_y, // first trade uses normal order
				second_dex_type,
				second_pool_type, //second_pool_type: u64,
				second_is_x_to_y, // second trade uses normal order
				third_dex_type,
				third_pool_type, //third_pool_type: u64,
				third_is_x_to_y, // second trade uses normal order
				coin_x  // x_in
				);


        assert!(coin::value(&coin_m) >= min_out_amount, E_NOT_SUFFICIENT);

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



	}


}
