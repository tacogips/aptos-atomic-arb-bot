script {

	use hippo_aggregator::aggregator;

	// only AMM AMM
	const POOL_TYPE :u64 = 0;


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
		aggregator::swap<
			0x1::aptos_coin::AptosCoin, //X
			Y,
			Z,
			0x1::aptos_coin::AptosCoin , // OutCoin
			C1,C2,C3>(signer,
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
				coin_amount_in, // x_in
				min_out_amount  //m_min_out
				);
	}


}
