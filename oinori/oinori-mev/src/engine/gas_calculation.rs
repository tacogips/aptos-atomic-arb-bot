use super::find_route::RouteLog;
use crate::coins::*;
use crate::hasher::*;
use crate::swap::router::SwapRouteStepInfo;
use crate::swap::SwapContract;
use once_cell::sync::Lazy;

#[derive(Debug, Copy, Clone)]
pub struct EstimateGas(pub u64);

const MIN_GAS_UNIT_PRICE: u64 = 100;
const MAX_GAS_UNIT_PRICE: u64 = 642_0300;

// average gas
const DEFAULT_ESTIMATE_MAX_GAS_TWO_STEP: u64 = 60;
const DEFAULT_ESTIMATE_MAX_GAS_THREE_STEP: u64 = 92;
pub const LOAN_GAS_AMOUNT: u64 = 5; // LOAN uses this gas amount

pub fn calc_gas_unit_price(
    estimate_gross_profit_without_gas_nor_loan_fee: u64,
    estimate_gas: EstimateGas,

    route_infos: &Vec<SwapRouteStepInfo>,
) -> u64 {
    let estimate_gas = estimate_gas.0;
    let equally_gax_price = estimate_gross_profit_without_gas_nor_loan_fee / estimate_gas;

    // Gas price calculation based on profit tiers
    let mut gas_unit_price = if estimate_gross_profit_without_gas_nor_loan_fee > 8_0000_0000 {
        equally_gax_price * 235 / 1000
    } else if estimate_gross_profit_without_gas_nor_loan_fee > 5_0000_0000 {
        equally_gax_price * 235 / 1000
    } else if estimate_gross_profit_without_gas_nor_loan_fee > 3_0000_0000 {
        equally_gax_price * 235 / 1000
    } else if estimate_gross_profit_without_gas_nor_loan_fee > 1_0000_0000 {
        equally_gax_price * 235 / 1000
    } else if estimate_gross_profit_without_gas_nor_loan_fee > 0_5000_0000 {
        equally_gax_price * 185 / 1000 //> 0.5
    } else if estimate_gross_profit_without_gas_nor_loan_fee > 0_1000_0000 {
        equally_gax_price * 105 / 1000 //> 0.1
    } else {
        equally_gax_price * 65 / 1000
    };

    if is_contains_thala(route_infos) {
        // reduce to 30%
        gas_unit_price = gas_unit_price * 30 / 100
    }

    // Reduce gas unit price
    gas_unit_price = gas_unit_price * 40 / 100;

    // Cap gas unit price at base amount
    let base_gas_unit = apt_amount(0.000011);
    if base_gas_unit < gas_unit_price {
        gas_unit_price = base_gas_unit;
    };

    let gas_unit_price = std::cmp::min(gas_unit_price, MAX_GAS_UNIT_PRICE);
    let gas_unit_price = std::cmp::max(gas_unit_price, MIN_GAS_UNIT_PRICE);
    gas_unit_price
}

fn is_contains_thala(route_infos: &Vec<SwapRouteStepInfo>) -> bool {
    for each in route_infos.iter() {
        if each.contract == &SwapContract::Thala {
            return true;
        }
    }

    false
}

pub fn estimate_max_gas(route_logs: &Vec<RouteLog>, use_loan: bool) -> u64 {
    let contracts_in_route: Vec<SwapContract> = route_logs
        .iter()
        .map(|each| each.contract.clone())
        .collect();

    let loan_gas = if use_loan { LOAN_GAS_AMOUNT } else { 0u64 };
    match ROUTE_TO_GAS_MAP.get(&contracts_in_route) {
        Some(estimated_gas) => *estimated_gas + loan_gas,
        None => {
            let mut thala_adjust_gas = 0;
            for each_route in route_logs.iter() {
                if each_route.contract == SwapContract::Thala {
                    thala_adjust_gas += thala_gas(&each_route.in_coin, &each_route.out_coin);
                }
            }

            if contracts_in_route.len() == 2 {
                DEFAULT_ESTIMATE_MAX_GAS_TWO_STEP + loan_gas + thala_adjust_gas
            } else {
                DEFAULT_ESTIMATE_MAX_GAS_THREE_STEP + loan_gas + thala_adjust_gas
            }
        },
    }
}

pub fn thala_gas(coin_x_id: &CoinId, coin_y_id: &CoinId) -> u64 {
    let apt_coin_id = apt_coin_id();
    let thala_mod_coin_id = thala_mod_coin_id();
    let thala_coin_id = thala_coin_id();
    let zusdc_coin_id = zusdt_coin_id();

    if (coin_x_id == &thala_mod_coin_id && coin_y_id == &thala_coin_id)
        || coin_y_id == &thala_mod_coin_id && coin_x_id == &thala_coin_id
    {
        // thl <-> mod
        310
    } else if (coin_x_id == &thala_mod_coin_id && coin_y_id == &apt_coin_id)
        || (coin_y_id == &thala_mod_coin_id && coin_x_id == &apt_coin_id)
    {
        // apt <-> mod
        310
    } else if (coin_x_id == &thala_coin_id && coin_y_id == &apt_coin_id)
        || (coin_y_id == &thala_coin_id && coin_x_id == &apt_coin_id)
    {
        // apt <-> thl
        310
    } else if (coin_x_id == &thala_mod_coin_id && coin_y_id == &zusdc_coin_id)
        || (coin_y_id == &zusdc_coin_id && coin_x_id == &thala_mod_coin_id)
    {
        // zusdc <-> mod
        100
    } else {
        700
    }
}

macro_rules! route_to_max_gas {
    ($dest:expr,$contract_1:expr, $contract_2:expr,$value:expr) => {
        $dest.insert(vec![$contract_1, $contract_2], $value)
    };

    ($dest:expr,$contract_1:expr, $contract_2:expr,$contract_3:expr, $value:expr) => {
        $dest.insert(vec![$contract_1, $contract_2, $contract_3], $value)
    };
}

#[rustfmt::skip]
pub static ROUTE_TO_GAS_MAP: Lazy<HashMap<Vec<SwapContract>, u64>> = Lazy::new(|| {
    let mut gas_map: HashMap<Vec<SwapContract>, u64> = HashMap::default();

    route_to_max_gas!(gas_map, SwapContract::Anime    ,SwapContract::AptoSwap ,37u64    );
    route_to_max_gas!(gas_map, SwapContract::Anime    ,SwapContract::Obric    ,53u64    );
    route_to_max_gas!(gas_map, SwapContract::Anime    ,SwapContract::Pancake  ,44u64    );
    route_to_max_gas!(gas_map, SwapContract::AptoSwap ,SwapContract::Obric    ,34u64    );
    route_to_max_gas!(gas_map, SwapContract::Aux      ,SwapContract::Anime    ,38u64    );
    route_to_max_gas!(gas_map, SwapContract::Aux      ,SwapContract::AptoSwap ,37u64    );
    route_to_max_gas!(gas_map, SwapContract::Liquid   ,SwapContract::AptoSwap ,74u64    );
    route_to_max_gas!(gas_map, SwapContract::Liquid   ,SwapContract::Obric    ,79u64    );
    route_to_max_gas!(gas_map, SwapContract::Liquid   ,SwapContract::Pancake  ,75u64    );
    route_to_max_gas!(gas_map, SwapContract::Obric    ,SwapContract::Anime    ,40u64    );
    route_to_max_gas!(gas_map, SwapContract::Obric    ,SwapContract::AptoSwap ,35u64    );
    route_to_max_gas!(gas_map, SwapContract::Obric    ,SwapContract::Liquid   ,74u64    );
    route_to_max_gas!(gas_map, SwapContract::Obric    ,SwapContract::Pancake  ,43u64    );
    route_to_max_gas!(gas_map, SwapContract::Pancake  ,SwapContract::Anime    ,52u64    );
    route_to_max_gas!(gas_map, SwapContract::Pancake  ,SwapContract::AptoSwap ,38u64    );
    route_to_max_gas!(gas_map, SwapContract::Pancake  ,SwapContract::Liquid   ,77u64    );
    route_to_max_gas!(gas_map, SwapContract::Pancake  ,SwapContract::Obric    ,41u64   );

    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::AptoSwap ,SwapContract::Liquid   ,75u64    );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Liquid   ,SwapContract::Liquid   ,138u64   );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Obric    ,SwapContract::Aux      ,62u64    );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Obric    ,SwapContract::Liquid   ,115u64   );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Obric    ,SwapContract::Pancake  ,56u64    );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Pancake  ,SwapContract::AptoSwap ,60u64    );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Pancake  ,SwapContract::Aux      ,85u64    );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Pancake  ,SwapContract::Liquid   ,105u64   );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Pancake  ,SwapContract::Obric    ,56u64    );
    route_to_max_gas!(gas_map,     SwapContract::Anime    ,SwapContract::Pancake  ,SwapContract::Pancake  ,56u64    );
    route_to_max_gas!(gas_map,     SwapContract::AptoSwap ,SwapContract::Liquid   ,SwapContract::Anime    ,92u64    );
    route_to_max_gas!(gas_map,     SwapContract::AptoSwap ,SwapContract::Liquid   ,SwapContract::AptoSwap ,87u64    );
    route_to_max_gas!(gas_map,     SwapContract::AptoSwap ,SwapContract::Liquid   ,SwapContract::Liquid   ,122u64   );
    route_to_max_gas!(gas_map,     SwapContract::AptoSwap ,SwapContract::Obric    ,SwapContract::Liquid   ,82u64    );
    route_to_max_gas!(gas_map,     SwapContract::AptoSwap ,SwapContract::Obric    ,SwapContract::Obric    ,47u64    );
    route_to_max_gas!(gas_map,     SwapContract::AptoSwap ,SwapContract::Pancake  ,SwapContract::Anime    ,58u64    );
    route_to_max_gas!(gas_map,     SwapContract::AptoSwap ,SwapContract::Pancake  ,SwapContract::Pancake  ,62u64    );
    route_to_max_gas!(gas_map,     SwapContract::Aux      ,SwapContract::Liquid   ,SwapContract::Liquid   ,110u64   );
    route_to_max_gas!(gas_map,     SwapContract::Aux      ,SwapContract::Liquid   ,SwapContract::Obric    ,97u64    );
    route_to_max_gas!(gas_map,     SwapContract::Aux      ,SwapContract::Obric    ,SwapContract::Obric    ,66u64    );
    route_to_max_gas!(gas_map,     SwapContract::Aux      ,SwapContract::Pancake  ,SwapContract::Pancake  ,58u64    );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Anime    ,SwapContract::Liquid   ,117u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Aux      ,SwapContract::Aux      ,138u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Liquid   ,SwapContract::Anime    ,132u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Liquid   ,SwapContract::AptoSwap ,124u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Liquid   ,SwapContract::Aux      ,110u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Liquid   ,SwapContract::Liquid   ,175u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Liquid   ,SwapContract::Obric    ,165u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Liquid   ,SwapContract::Pancake  ,165u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Obric    ,SwapContract::Anime    ,108u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Obric    ,SwapContract::Liquid   ,163u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Obric    ,SwapContract::Obric    ,95u64    );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Obric    ,SwapContract::Pancake  ,98u64    );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Pancake  ,SwapContract::Anime    ,89u64    );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Pancake  ,SwapContract::Liquid   ,164u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Pancake  ,SwapContract::Obric    ,114u64   );
    route_to_max_gas!(gas_map,     SwapContract::Liquid   ,SwapContract::Pancake  ,SwapContract::Pancake  ,111u64   );
    route_to_max_gas!(gas_map,     SwapContract::Obric    ,SwapContract::Liquid   ,SwapContract::Obric    ,90u64    );
    route_to_max_gas!(gas_map,     SwapContract::Obric    ,SwapContract::Obric    ,SwapContract::AptoSwap ,47u64    );
    route_to_max_gas!(gas_map,     SwapContract::Obric    ,SwapContract::Obric    ,SwapContract::Liquid   ,84u64    );
    route_to_max_gas!(gas_map,     SwapContract::Obric    ,SwapContract::Obric    ,SwapContract::Obric    ,44u64    );
    route_to_max_gas!(gas_map,     SwapContract::Obric    ,SwapContract::Obric    ,SwapContract::Pancake  ,50u64    );
    route_to_max_gas!(gas_map,     SwapContract::Obric    ,SwapContract::Pancake  ,SwapContract::Liquid   ,123u64   );
    route_to_max_gas!(gas_map,     SwapContract::Obric    ,SwapContract::Pancake  ,SwapContract::Obric    ,53u64    );
    route_to_max_gas!(gas_map,     SwapContract::Obric    ,SwapContract::Pancake  ,SwapContract::Pancake  ,56u64    );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Liquid   ,SwapContract::Liquid   ,165u64   );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Obric    ,SwapContract::AptoSwap ,52u64    );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Obric    ,SwapContract::Liquid   ,102u64   );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Obric    ,SwapContract::Obric    ,53u64    );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Obric    ,SwapContract::Pancake  ,62u64    );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Pancake  ,SwapContract::Anime    ,54u64    );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Pancake  ,SwapContract::AptoSwap ,53u64    );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Pancake  ,SwapContract::Liquid   ,99u64    );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Pancake  ,SwapContract::Obric    ,63u64    );
    route_to_max_gas!(gas_map,     SwapContract::Pancake  ,SwapContract::Pancake  ,SwapContract::Pancake  ,64u64    );

    gas_map

});

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_calc_gas_unit_price() {
        let estimate_profit: u64 = 1342413073 - 1234113002 - 3702339;
        let estimate_gas = EstimateGas(68);
        let gas_price = calc_gas_unit_price(estimate_profit, estimate_gas, &vec![]);
        // should be over 2605

        println!("estimated profit before gas:{}", estimate_profit);
        println!(
            "estimated profit after gas :{}",
            estimate_profit - gas_price * estimate_gas.0
        );
        println!("gas price:{}", gas_price);
        println!("estimate gas :{}", gas_price * estimate_gas.0);

        assert!(gas_price > 1000);
        assert!(estimate_profit > gas_price * estimate_gas.0);
        assert!(estimate_profit - gas_price * estimate_gas.0 >= 7800_0000);
    }
}
