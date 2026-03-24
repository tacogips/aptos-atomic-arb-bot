use crate::coins::{aptoge_coin_id, whbtc_coin_id, zusdc_coin_id, zusdt_coin_id, CoinId};
use crate::swap::amm::AmmCurveType;
use crate::swap::router::*;
use crate::swap::SwapContract;
use crate::transaction::builder::*;

enum DexCode {
    //ECONIA,
    PONTEM,
    //BASIQ,
    //DITTO,
    //TORTUGA,
    APTOSWAP,
    AUX,
    ANIMESWAP,
    CETUS,
    PANCAKE,
    OBRIC,
    THALA,
}

impl DexCode {
    fn as_value(&self) -> u8 {
        match self {
            //Self::ECONIA => 2,
            Self::PONTEM => 3,
            //Self::BASIQ => 4,
            //Self::DITTO => 5,
            //Self::TORTUGA => 6,
            Self::APTOSWAP => 7,
            Self::AUX => 8,
            Self::ANIMESWAP => 9,
            Self::CETUS => 10,
            Self::PANCAKE => 11,
            Self::OBRIC => 12,
            Self::THALA => 14,
        }
    }
}

struct ArgAndTypeForSwap {
    dex_type: FunctionArg,
    is_x_to_y: FunctionArg,
    pool_type: FunctionArg,
    output_coin_type: FunctionTypeTag,
    curve_type: FunctionTypeTag,
}

impl Default for ArgAndTypeForSwap {
    fn default() -> Self {
        Self {
            dex_type: FunctionArg::u8(0),
            is_x_to_y: FunctionArg::bool(true),
            pool_type: FunctionArg::u64(0),
            output_coin_type: FunctionTypeTag::coin(zusdc_coin_id()),
            curve_type: FunctionTypeTag::curve_type(None),
        }
    }
}

struct DexSwapInfo {
    dex_code: DexCode,
    output_coin_id: CoinId,
    is_x_to_y: bool,
    curve_type: Option<CurveTypeTag>,
    pool_type: u64,
}

impl DexSwapInfo {
    fn as_arg_and_type(self) -> ArgAndTypeForSwap {
        ArgAndTypeForSwap {
            dex_type: FunctionArg::u8(self.dex_code.as_value()),
            is_x_to_y: FunctionArg::bool(self.is_x_to_y),
            pool_type: FunctionArg::u64(self.pool_type),
            output_coin_type: FunctionTypeTag::coin(self.output_coin_id),
            curve_type: FunctionTypeTag::curve_type(self.curve_type),
        }
    }

    fn from_route(swap_route_step: &SwapRouteStepInfo<'_>) -> Self {
        let curve_type = convert_swap_curve_type_tag(
            swap_route_step.contract,
            swap_route_step.swap_pair_info.amm_curve_type,
        );

        let output_coin_id = swap_route_step.swap_pair_info.out_coin.coin_id.clone();

        let dex_code = match swap_route_step.contract {
            SwapContract::Liquid => DexCode::PONTEM,
            SwapContract::Aux => DexCode::AUX,
            SwapContract::Anime => DexCode::ANIMESWAP,
            SwapContract::Pancake => DexCode::PANCAKE,
            SwapContract::Cetus => DexCode::CETUS,
            SwapContract::Obric => DexCode::OBRIC,
            SwapContract::AptoSwap => DexCode::APTOSWAP,
            SwapContract::Thala => DexCode::THALA,
        };

        Self {
            dex_code,
            output_coin_id,
            is_x_to_y: swap_route_step.is_x_to_y,
            curve_type,
            pool_type: hippo_pool_type(swap_route_step),
        }
    }
}

fn hippo_pool_type(swap_route_step: &SwapRouteStepInfo) -> u64 {
    if let AmmCurveType::ThalaAmm(thala_param) = swap_route_step.swap_pair_info.amm_curve_type {
        thala_param.as_hippo_pool_type()
    } else {
        0
    }
}

pub enum LoanCounterCoin {
    Usdc,
    Usdt,
    Btc,
    Aptoge,
}

pub fn convert_hippo_args_v1(
    input_amount: u64,
    route_infos: &Vec<SwapRouteStepInfo<'_>>,
    ensure_min_out: bool,
    loan_counter_coin: LoanCounterCoin,
) -> (Vec<FunctionArg>, Vec<FunctionTypeTag>) {
    let dex_swap_infos: Vec<ArgAndTypeForSwap> = route_infos
        .into_iter()
        .map(|route_info| DexSwapInfo::from_route(route_info).as_arg_and_type())
        .collect();

    let coin_amount_in = FunctionArg::u64(input_amount);
    let min_out_amount_in = if ensure_min_out {
        FunctionArg::u64(input_amount)
    } else {
        FunctionArg::u64(0)
    };
    let num_steps = FunctionArg::u8(dex_swap_infos.len() as u8);

    let mut dex_swap_infos = dex_swap_infos.into_iter();
    let (first_dex_type, first_is_x_to_y, first_coin_out_type, first_curve_type) = {
        let swap = dex_swap_infos.next().unwrap_or_default();
        (
            swap.dex_type,
            swap.is_x_to_y,
            swap.output_coin_type,
            swap.curve_type,
        )
    };

    let (second_dex_type, second_is_x_to_y, second_coin_out_type, second_curve_type) = {
        let swap = dex_swap_infos.next().unwrap_or_default();
        (
            swap.dex_type,
            swap.is_x_to_y,
            swap.output_coin_type,
            swap.curve_type,
        )
    };

    let (third_dex_type, third_is_x_to_y, _third_coin_out_type, third_curve_type) = {
        let swap = dex_swap_infos.next().unwrap_or_default();
        (
            swap.dex_type,
            swap.is_x_to_y,
            swap.output_coin_type,
            swap.curve_type,
        )
    };

    let loan_counter_coin_id = match loan_counter_coin {
        LoanCounterCoin::Usdc => zusdc_coin_id(),
        LoanCounterCoin::Usdt => zusdt_coin_id(),
        LoanCounterCoin::Btc => whbtc_coin_id(),
        LoanCounterCoin::Aptoge => aptoge_coin_id(),
    };

    let args = vec![
        coin_amount_in,    //		coin_amount_in:u64,
        min_out_amount_in, //		min_out_amount_in:u64,
        num_steps,         //		num_steps:u8,
        first_dex_type,    //		first_dex_type:u8,
        first_is_x_to_y,   //		first_is_x_to_y:bool,
        second_dex_type,   //		second_dex_type:u8,
        second_is_x_to_y,  //		second_is_x_to_y:bool,
        third_dex_type,    //		third_dex_type:u8,
        third_is_x_to_y,   //		third_is_x_to_y:bool);
    ];

    let type_tags = vec![
        first_coin_out_type,                                                 //Y
        second_coin_out_type,                                                //Z
        first_curve_type,                                                    // C1
        second_curve_type,                                                   // C2
        third_curve_type,                                                    // C3
        FunctionTypeTag::coin(loan_counter_coin_id),                         // BY
        FunctionTypeTag::curve_type(Some(CurveTypeTag::LiquidUncorrelated)), // BC
    ];
    (args, type_tags)
}

pub fn convert_hippo_args_v2(
    input_amount: u64,
    route_infos: &Vec<SwapRouteStepInfo<'_>>,
    ensure_min_out: bool,
    loan_counter_coin: LoanCounterCoin,
) -> (Vec<FunctionArg>, Vec<FunctionTypeTag>) {
    let dex_swap_infos: Vec<ArgAndTypeForSwap> = route_infos
        .into_iter()
        .map(|route_info| DexSwapInfo::from_route(route_info).as_arg_and_type())
        .collect();

    let coin_amount_in = FunctionArg::u64(input_amount);
    let min_out_amount_in = if ensure_min_out {
        FunctionArg::u64(input_amount)
    } else {
        FunctionArg::u64(0)
    };
    let num_steps = FunctionArg::u8(dex_swap_infos.len() as u8);

    let mut dex_swap_infos = dex_swap_infos.into_iter();
    let (first_dex_type, first_is_x_to_y, first_pool_type, first_coin_out_type, first_curve_type) = {
        let swap = dex_swap_infos.next().unwrap_or_default();
        (
            swap.dex_type,
            swap.is_x_to_y,
            swap.pool_type,
            swap.output_coin_type,
            swap.curve_type,
        )
    };

    let (
        second_dex_type,
        second_is_x_to_y,
        second_pool_type,
        second_coin_out_type,
        second_curve_type,
    ) = {
        let swap = dex_swap_infos.next().unwrap_or_default();
        (
            swap.dex_type,
            swap.is_x_to_y,
            swap.pool_type,
            swap.output_coin_type,
            swap.curve_type,
        )
    };

    let (third_dex_type, third_is_x_to_y, third_pool_type, _third_coin_out_type, third_curve_type) = {
        let swap = dex_swap_infos.next().unwrap_or_default();
        (
            swap.dex_type,
            swap.is_x_to_y,
            swap.pool_type,
            swap.output_coin_type,
            swap.curve_type,
        )
    };

    let loan_counter_coin_id = match loan_counter_coin {
        LoanCounterCoin::Usdc => zusdc_coin_id(),
        LoanCounterCoin::Usdt => zusdt_coin_id(),
        LoanCounterCoin::Btc => whbtc_coin_id(),
        LoanCounterCoin::Aptoge => aptoge_coin_id(),
    };

    let args = vec![
        coin_amount_in,    //		coin_amount_in:u64,
        min_out_amount_in, //		min_out_amount_in:u64,
        num_steps,         //		num_steps:u8,
        first_dex_type,    //		first_dex_type:u8,
        first_is_x_to_y,   //		first_is_x_to_y:bool,
        first_pool_type,   //		first_pool_type
        second_dex_type,   //		second_dex_type:u8,
        second_is_x_to_y,  //		second_is_x_to_y:bool,
        second_pool_type,  //		second pool_type
        third_dex_type,    //		third_dex_type:u8,
        third_is_x_to_y,   //		third_is_x_to_y:bool);
        third_pool_type,   //		third_pool_type
    ];

    let type_tags = vec![
        first_coin_out_type,                                                 //Y
        second_coin_out_type,                                                //Z
        first_curve_type,                                                    // C1
        second_curve_type,                                                   // C2
        third_curve_type,                                                    // C3
        FunctionTypeTag::coin(loan_counter_coin_id),                         // BY
        FunctionTypeTag::curve_type(Some(CurveTypeTag::LiquidUncorrelated)), // BC
    ];
    (args, type_tags)
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::coins::{apt_coin_id, CoinId};
    use std::path::PathBuf;

    fn load_router_pool() -> SwapRouterPool {
        let mut pb = PathBuf::new();
        let test_data_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        pb.push(test_data_dir);
        pb.push("test-data");
        pb.push("router_pool_75584877.json");

        let f = std::fs::OpenOptions::new().read(true).open(pb).unwrap();
        serde_json::from_reader(f).unwrap()
    }

    fn load_route(file_name: &str) -> SwapRoute {
        let mut pb = PathBuf::new();
        let test_data_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        pb.push(test_data_dir);
        pb.push("test-data");
        pb.push(format!("{}.json", file_name));

        let f = std::fs::OpenOptions::new().read(true).open(pb).unwrap();
        serde_json::from_reader(f).unwrap()
    }

    #[tokio::test]
    async fn test_convert_hippo_args1_v1() {
        let router_pool = load_router_pool();

        let route = load_route("route_1");
        let input_amount = route.input_amount;
        let route_info = router_pool.convert_swap_route_infos(route);
        println!("-- {}", serde_json::to_string_pretty(&route_info).unwrap());

        let (args, types) =
            convert_hippo_args_v1(input_amount, &route_info, true, LoanCounterCoin::Usdc);

        let coin_amount_in = FunctionArg::u64(input_amount);
        let min_out_amount_in = FunctionArg::u64(input_amount);
        let num_steps = FunctionArg::u8(2);
        let first_dex_type = FunctionArg::u8(9);
        let first_is_x_to_y = FunctionArg::bool(true);

        let second_dex_type = FunctionArg::u8(12);
        let second_is_x_to_y = FunctionArg::bool(false);

        let third_dex_type = FunctionArg::u8(0);
        let third_is_x_to_y = FunctionArg::bool(true);

        let z_usdc = zusdc_coin_id();
        let expected_args = vec![
            coin_amount_in,    //		coin_amount_in:u64,
            min_out_amount_in, //		min_out_amount_in:u64,
            num_steps,         //		num_steps:u8,
            first_dex_type,    //		first_dex_type:u8,
            first_is_x_to_y,   //		first_is_x_to_y:bool,
            second_dex_type,   //		second_dex_type:u8,
            second_is_x_to_y,  //		second_is_x_to_y:bool,
            third_dex_type,    //		third_dex_type:u8,
            third_is_x_to_y,   //		third_is_x_to_y:bool);
        ];

        let first_coin_out_type = FunctionTypeTag::coin(z_usdc.clone()); //Y
        let second_coin_out_type = FunctionTypeTag::coin(apt_coin_id()); //Z
        let first_curve_type = FunctionTypeTag::curve_type(None); // C1
        let second_curve_type = FunctionTypeTag::curve_type(None); // C2
        let third_curve_type = FunctionTypeTag::curve_type(None); // C3

        let expected_type_tags = vec![
            first_coin_out_type,                                                 //Y
            second_coin_out_type,                                                //Z
            first_curve_type,                                                    // C1
            second_curve_type,                                                   // C2
            third_curve_type,                                                    // C3
            FunctionTypeTag::coin(z_usdc),                                       // BY
            FunctionTypeTag::curve_type(Some(CurveTypeTag::LiquidUncorrelated)), // BC
        ];

        assert_eq!(args, expected_args);
        assert_eq!(types, expected_type_tags);
    }

    #[tokio::test]
    async fn test_convert_hippo_args2_v1() {
        let router_pool = load_router_pool();

        let route = load_route("route_2");
        let input_amount = route.input_amount;
        let route_info = router_pool.convert_swap_route_infos(route);
        println!("-- {}", serde_json::to_string_pretty(&route_info).unwrap());

        let (args, types) =
            convert_hippo_args_v1(input_amount, &route_info, true, LoanCounterCoin::Usdc);

        let wh_usdc = CoinId::from_addresses_str(
            "0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea",
            "coin",
            "T",
        );

        let coin_amount_in = FunctionArg::u64(input_amount);
        let min_out_amount_in = FunctionArg::u64(input_amount);
        let num_steps = FunctionArg::u8(3);
        let first_dex_type = FunctionArg::u8(12);
        let first_is_x_to_y = FunctionArg::bool(true);

        let second_dex_type = FunctionArg::u8(12);
        let second_is_x_to_y = FunctionArg::bool(true);

        let third_dex_type = FunctionArg::u8(7);
        let third_is_x_to_y = FunctionArg::bool(false);

        let z_usdc = zusdc_coin_id();
        let expected_args = vec![
            coin_amount_in,    //		coin_amount_in:u64,
            min_out_amount_in, //		min_out_amount_in:u64,
            num_steps,         //		num_steps:u8,
            first_dex_type,    //		first_dex_type:u8,
            first_is_x_to_y,   //		first_is_x_to_y:bool,
            second_dex_type,   //		second_dex_type:u8,
            second_is_x_to_y,  //		second_is_x_to_y:bool,
            third_dex_type,    //		third_dex_type:u8,
            third_is_x_to_y,   //		third_is_x_to_y:bool);
        ];

        let first_coin_out_type = FunctionTypeTag::coin(wh_usdc); //Y
        let second_coin_out_type = FunctionTypeTag::coin(z_usdc.clone()); //Z
                                                                          //
        let first_curve_type = FunctionTypeTag::curve_type(None); // C1
        let second_curve_type = FunctionTypeTag::curve_type(None); // C2
        let third_curve_type = FunctionTypeTag::curve_type(None); // C3

        let expected_type_tags = vec![
            first_coin_out_type,                                                 //Y
            second_coin_out_type,                                                //Z
            first_curve_type,                                                    // C1
            second_curve_type,                                                   // C2
            third_curve_type,                                                    // C3
            FunctionTypeTag::coin(z_usdc),                                       // BY
            FunctionTypeTag::curve_type(Some(CurveTypeTag::LiquidUncorrelated)), // BC
        ];

        assert_eq!(args, expected_args);
        assert_eq!(types, expected_type_tags);
    }

    #[tokio::test]
    async fn test_convert_hippo_args3_v1() {
        let router_pool = load_router_pool();

        let route = load_route("route_2");
        let input_amount = route.input_amount;
        let route_info = router_pool.convert_swap_route_infos(route);
        println!("-- {}", serde_json::to_string_pretty(&route_info).unwrap());

        let (args, types) =
            convert_hippo_args_v1(input_amount, &route_info, true, LoanCounterCoin::Usdt);

        let wh_usdc = CoinId::from_addresses_str(
            "0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea",
            "coin",
            "T",
        );

        let coin_amount_in = FunctionArg::u64(input_amount);
        let min_out_amount_in = FunctionArg::u64(input_amount);
        let num_steps = FunctionArg::u8(3);
        let first_dex_type = FunctionArg::u8(12);
        let first_is_x_to_y = FunctionArg::bool(true);

        let second_dex_type = FunctionArg::u8(12);
        let second_is_x_to_y = FunctionArg::bool(true);

        let third_dex_type = FunctionArg::u8(7);
        let third_is_x_to_y = FunctionArg::bool(false);

        let z_usdc = zusdc_coin_id();
        let expected_args = vec![
            coin_amount_in,    //		coin_amount_in:u64,
            min_out_amount_in, //		min_out_amount_in:u64,
            num_steps,         //		num_steps:u8,
            first_dex_type,    //		first_dex_type:u8,
            first_is_x_to_y,   //		first_is_x_to_y:bool,
            second_dex_type,   //		second_dex_type:u8,
            second_is_x_to_y,  //		second_is_x_to_y:bool,
            third_dex_type,    //		third_dex_type:u8,
            third_is_x_to_y,   //		third_is_x_to_y:bool);
        ];

        let first_coin_out_type = FunctionTypeTag::coin(wh_usdc); //Y
        let second_coin_out_type = FunctionTypeTag::coin(z_usdc.clone()); //Z
                                                                          //
        let first_curve_type = FunctionTypeTag::curve_type(None); // C1
        let second_curve_type = FunctionTypeTag::curve_type(None); // C2
        let third_curve_type = FunctionTypeTag::curve_type(None); // C3

        let z_usdt = zusdt_coin_id();
        let expected_type_tags = vec![
            first_coin_out_type,                                                 //Y
            second_coin_out_type,                                                //Z
            first_curve_type,                                                    // C1
            second_curve_type,                                                   // C2
            third_curve_type,                                                    // C3
            FunctionTypeTag::coin(z_usdt),                                       // BY
            FunctionTypeTag::curve_type(Some(CurveTypeTag::LiquidUncorrelated)), // BC
        ];

        assert_eq!(args, expected_args);
        assert_eq!(types, expected_type_tags);
    }

    #[tokio::test]
    async fn test_convert_hippo_args1_v2() {
        let router_pool = load_router_pool();

        let route = load_route("route_1");
        let input_amount = route.input_amount;
        let route_info = router_pool.convert_swap_route_infos(route);
        println!("-- {}", serde_json::to_string_pretty(&route_info).unwrap());

        let (args, types) =
            convert_hippo_args_v2(input_amount, &route_info, true, LoanCounterCoin::Usdc);

        let coin_amount_in = FunctionArg::u64(input_amount);
        let min_out_amount_in = FunctionArg::u64(input_amount);
        let num_steps = FunctionArg::u8(2);
        let first_dex_type = FunctionArg::u8(9);
        let first_is_x_to_y = FunctionArg::bool(true);
        let first_pool_type = FunctionArg::u64(0);

        let second_dex_type = FunctionArg::u8(12);
        let second_is_x_to_y = FunctionArg::bool(false);
        let second_pool_type = FunctionArg::u64(0);

        let third_dex_type = FunctionArg::u8(0);
        let third_is_x_to_y = FunctionArg::bool(true);
        let third_pool_type = FunctionArg::u64(0);

        let z_usdc = zusdc_coin_id();
        let expected_args = vec![
            coin_amount_in,    //		coin_amount_in:u64,
            min_out_amount_in, //		min_out_amount_in:u64,
            num_steps,         //		num_steps:u8,
            first_dex_type,    //		first_dex_type:u8,
            first_is_x_to_y,   //		first_is_x_to_y:bool,
            first_pool_type,
            second_dex_type,  //		second_dex_type:u8,
            second_is_x_to_y, //		second_is_x_to_y:bool,
            //
            second_pool_type,
            third_dex_type,  //		third_dex_type:u8,
            third_is_x_to_y, //		third_is_x_to_y:bool);
            //
            third_pool_type,
        ];

        let first_coin_out_type = FunctionTypeTag::coin(z_usdc.clone()); //Y
        let second_coin_out_type = FunctionTypeTag::coin(apt_coin_id()); //Z
        let first_curve_type = FunctionTypeTag::curve_type(None); // C1
        let second_curve_type = FunctionTypeTag::curve_type(None); // C2
        let third_curve_type = FunctionTypeTag::curve_type(None); // C3

        let expected_type_tags = vec![
            first_coin_out_type,                                                 //Y
            second_coin_out_type,                                                //Z
            first_curve_type,                                                    // C1
            second_curve_type,                                                   // C2
            third_curve_type,                                                    // C3
            FunctionTypeTag::coin(z_usdc),                                       // BY
            FunctionTypeTag::curve_type(Some(CurveTypeTag::LiquidUncorrelated)), // BC
        ];

        assert_eq!(args, expected_args);
        assert_eq!(types, expected_type_tags);
    }

    #[tokio::test]
    async fn test_convert_hippo_args2_v2() {
        let router_pool = load_router_pool();

        let route = load_route("route_2");
        let input_amount = route.input_amount;
        let route_info = router_pool.convert_swap_route_infos(route);
        println!("-- {}", serde_json::to_string_pretty(&route_info).unwrap());

        let (args, types) =
            convert_hippo_args_v2(input_amount, &route_info, true, LoanCounterCoin::Usdc);

        let wh_usdc = CoinId::from_addresses_str(
            "0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea",
            "coin",
            "T",
        );

        let coin_amount_in = FunctionArg::u64(input_amount);
        let min_out_amount_in = FunctionArg::u64(input_amount);
        let num_steps = FunctionArg::u8(3);
        let first_dex_type = FunctionArg::u8(12);
        let first_is_x_to_y = FunctionArg::bool(true);
        let first_pool_type = FunctionArg::u64(0);

        let second_dex_type = FunctionArg::u8(12);
        let second_is_x_to_y = FunctionArg::bool(true);

        let second_pool_type = FunctionArg::u64(0);

        let third_dex_type = FunctionArg::u8(7);
        let third_is_x_to_y = FunctionArg::bool(false);
        let third_pool_type = FunctionArg::u64(0);

        let z_usdc = zusdc_coin_id();
        let expected_args = vec![
            coin_amount_in,    //		coin_amount_in:u64,
            min_out_amount_in, //		min_out_amount_in:u64,
            num_steps,         //		num_steps:u8,
            first_dex_type,    //		first_dex_type:u8,
            first_is_x_to_y,   //		first_is_x_to_y:bool,
            first_pool_type,   //
            second_dex_type,   //		second_dex_type:u8,
            second_is_x_to_y,  //		second_is_x_to_y:bool,
            second_pool_type,
            third_dex_type,  //		third_dex_type:u8,
            third_is_x_to_y, //		third_is_x_to_y:bool);
            third_pool_type,
        ];

        let first_coin_out_type = FunctionTypeTag::coin(wh_usdc); //Y
        let second_coin_out_type = FunctionTypeTag::coin(z_usdc.clone()); //Z
                                                                          //
        let first_curve_type = FunctionTypeTag::curve_type(None); // C1
        let second_curve_type = FunctionTypeTag::curve_type(None); // C2
        let third_curve_type = FunctionTypeTag::curve_type(None); // C3

        let expected_type_tags = vec![
            first_coin_out_type,                                                 //Y
            second_coin_out_type,                                                //Z
            first_curve_type,                                                    // C1
            second_curve_type,                                                   // C2
            third_curve_type,                                                    // C3
            FunctionTypeTag::coin(z_usdc),                                       // BY
            FunctionTypeTag::curve_type(Some(CurveTypeTag::LiquidUncorrelated)), // BC
        ];

        assert_eq!(args, expected_args);
        assert_eq!(types, expected_type_tags);
    }

    #[tokio::test]
    async fn test_convert_hippo_args3_v2() {
        let router_pool = load_router_pool();

        let route = load_route("route_2");
        let input_amount = route.input_amount;
        let route_info = router_pool.convert_swap_route_infos(route);
        println!("-- {}", serde_json::to_string_pretty(&route_info).unwrap());

        let (args, types) =
            convert_hippo_args_v2(input_amount, &route_info, true, LoanCounterCoin::Usdt);

        let wh_usdc = CoinId::from_addresses_str(
            "0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea",
            "coin",
            "T",
        );

        let coin_amount_in = FunctionArg::u64(input_amount);
        let min_out_amount_in = FunctionArg::u64(input_amount);
        let num_steps = FunctionArg::u8(3);
        let first_dex_type = FunctionArg::u8(12);
        let first_is_x_to_y = FunctionArg::bool(true);
        let first_pool_type = FunctionArg::u64(0);

        let second_dex_type = FunctionArg::u8(12);
        let second_is_x_to_y = FunctionArg::bool(true);
        let second_pool_type = FunctionArg::u64(0);

        let third_dex_type = FunctionArg::u8(7);
        let third_is_x_to_y = FunctionArg::bool(false);
        let third_pool_type = FunctionArg::u64(0);

        let z_usdc = zusdc_coin_id();
        let expected_args = vec![
            coin_amount_in,    //		coin_amount_in:u64,
            min_out_amount_in, //		min_out_amount_in:u64,
            num_steps,         //		num_steps:u8,
            first_dex_type,    //		first_dex_type:u8,
            first_is_x_to_y,   //		first_is_x_to_y:bool,
            first_pool_type,
            second_dex_type,  //		second_dex_type:u8,
            second_is_x_to_y, //		second_is_x_to_y:bool,
            second_pool_type,
            third_dex_type,  //		third_dex_type:u8,
            third_is_x_to_y, //		third_is_x_to_y:bool);
            third_pool_type,
        ];

        let first_coin_out_type = FunctionTypeTag::coin(wh_usdc); //Y
        let second_coin_out_type = FunctionTypeTag::coin(z_usdc.clone()); //Z
                                                                          //
        let first_curve_type = FunctionTypeTag::curve_type(None); // C1
        let second_curve_type = FunctionTypeTag::curve_type(None); // C2
        let third_curve_type = FunctionTypeTag::curve_type(None); // C3

        let z_usdt = zusdt_coin_id();
        let expected_type_tags = vec![
            first_coin_out_type,                                                 //Y
            second_coin_out_type,                                                //Z
            first_curve_type,                                                    // C1
            second_curve_type,                                                   // C2
            third_curve_type,                                                    // C3
            FunctionTypeTag::coin(z_usdt),                                       // BY
            FunctionTypeTag::curve_type(Some(CurveTypeTag::LiquidUncorrelated)), // BC
        ];

        assert_eq!(args, expected_args);
        assert_eq!(types, expected_type_tags);
    }
}
