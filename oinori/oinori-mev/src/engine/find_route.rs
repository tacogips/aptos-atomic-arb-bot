use aptos_logger::prelude::*;
use super::cache::*;
use crate::aggregator;
use crate::coins::{
    apt_coin_id, aptoge_coin_id, whbtc_coin_id, zusdc_coin_id, zusdt_coin_id, CoinId,
};
use crate::engine::gas_calculation::*;
use crate::swap::{
    optimize_route_amount_with_loan_fee, start_route_traverse, RoutePairFilter, SwapContract,
    SwapRoute, SwapRouterPool, UpdatePairType,
};

use crate::transaction::builder::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::time::Duration as TokioDuration;

fn specific_max_gas_amount(estimated_gas_amount: u64) -> u64 {
    estimated_gas_amount + 200 // 200 is buffer
}
const MAX_INPUT_APT_AMOUNT: u64 = 5000_0000_0000; //max 5000 apt

pub async fn find_best_route(
    swap_router_pool: &SwapRouterPool,
    input_amounts: Vec<u64>,
    swap_filters_for_steps: Option<(
        Option<SwapContract>,
        Option<SwapContract>,
        Option<SwapContract>,
    )>,
    coin_filters_for_steps: (
        Vec<RoutePairFilter>,
        Vec<RoutePairFilter>,
        Vec<RoutePairFilter>,
    ),
    exclude_in_all_route_swap: Option<SwapContract>,
) -> Result<Option<SwapRoute>> {
    let timeout = Some(TokioDuration::from_millis(15));
    let max_depth = Some(3u8); //

    start_route_traverse(
        swap_router_pool,
        apt_coin_id(),
        input_amounts,
        timeout,
        max_depth,
        swap_filters_for_steps,
        coin_filters_for_steps,
        exclude_in_all_route_swap,
    )
    .await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRouteResult {
    target_version: u64,
    traverse_start_at: DateTime<Utc>,
    traverse_duration_micro: i64,

    optimize_start_at: DateTime<Utc>,
    optimize_duration_micro: i64,

    pub original_input_amount: u64,
    pub original_output_amount: u64,

    pub input_amount: u64,
    estimated_max_gas: u64,
    estimated_gas_fee: u64,
    loan_fee: u64,
    output_amount: u64,
    profit_amount: u64,

    pub gas_unit_price: u64,
    pub use_loan: bool,
    pub specified_max_gas_amount: Option<u64>,
    route_logs: Vec<RouteLog>,

    threshold_profit_amount: u64,
}

impl SearchRouteResult {
    pub fn as_cache_key(&self) -> RecentFiredMevKey {
        RecentFiredMevKey {
            input_amount: self.input_amount,
            output_amount: self.output_amount,

            contracts: self
                .route_logs
                .iter()
                .map(|each| each.contract.clone())
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CandidateRouteResult {
    target_version: u64,
    traverse_start_at: DateTime<Utc>,
    traverse_duration_micro: i64,

    affected_swap_updates: Vec<(SwapContract, UpdatePairType)>,
    optimize_start_at: DateTime<Utc>,
    optimize_duration_micro: i64,
    gas_unit_price: u64,

    pub original_input_amount: u64,
    pub original_output_amount: u64,

    input_amount: u64,
    estimated_max_gas: u64,

    estimated_gas_fee: u64,
    loan_fee: u64,
    output_amount: u64,
    profit_amount: u64,
    route_logs: Vec<RouteLog>,

    threshold_profit_amount: u64,

    pub swap_filters_for_steps: Option<(
        Option<SwapContract>,
        Option<SwapContract>,
        Option<SwapContract>,
    )>,
    pub coin_filters_for_steps: (
        Vec<RoutePairFilter>,
        Vec<RoutePairFilter>,
        Vec<RoutePairFilter>,
    ),
    pub input_amounts: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RouteLog {
    pub contract: SwapContract,
    pub in_coin: CoinId,
    pub out_coin: CoinId,
    pub is_x_to_y: bool,
    pub input_amount: u64,
    pub output_amount: u64,
}

pub struct ContractArgs {
    pub contract_func_args: Vec<FunctionArg>,
    pub contract_func_types: Vec<FunctionTypeTag>,
}

#[derive(Debug, Clone, Copy)]
pub struct MaxGasForTwoStep(pub u64);

#[derive(Debug, Clone, Copy)]
pub struct MaxGasForThreeStep(pub u64);

pub struct UseLoanAmount(pub u64);
pub struct ProfitAmout(pub u64);
#[derive(Clone, Copy)]
pub struct DurationMicroSec(pub u64);

const LINEAR_OPTIMIZE_STEP_RANGE_AMOUNT: u64 = 3000_0000; // 0.3 apt
const LINEAR_OPTIMIZE_STEP_MAX_STEP: usize = 500; // 0.3 * 500 = 150 apt

fn loan_info(
    best_route: &SwapRoute,
    swap_router_pool: &SwapRouterPool,
    not_use_loan_max_amount: u64,
) -> (u64, aggregator::LoanCounterCoin) {
    let zusdc_coin = zusdc_coin_id();
    let zusdt_coin = zusdt_coin_id();
    let wh_btc_coin = whbtc_coin_id();
    let aptoge_coin = aptoge_coin_id();
    let apt_coin = apt_coin_id();

    let liquid_swap_index = swap_router_pool.get_swap_index(&SwapContract::Liquid);
    let (has_usdc_pair, has_usdt_pair, has_btc_pair, has_aptoge_pair) = match liquid_swap_index {
        None => (false, false, false, false),
        Some(liquid_index) => {
            let mut has_usdc_pair = false;
            let mut has_usdt_pair = false;
            let mut has_btc_pair = false;
            let mut has_aptoge_pair = false;

            for each_liquid_step in best_route.find_by_swap_index(liquid_index) {
                let reserved_conin_id = unsafe {
                    swap_router_pool.get_pair_by_swap_index_unchecked(
                        &each_liquid_step.swap_index_and_direction.swap_pair_index,
                    )
                };
                // supposed use flash loan (x = zusdc, y=aptos) pair
                if reserved_conin_id.coin_ids() == (&zusdc_coin, &apt_coin) {
                    has_usdc_pair = true;
                }

                // supposed use flash loan (x = zusdc, y=aptos) pair
                if reserved_conin_id.coin_ids() == (&zusdt_coin, &apt_coin) {
                    has_usdt_pair = true;
                }

                // supposed use flash loan (x = zusdc, y=aptos) pair
                if reserved_conin_id.coin_ids() == (&wh_btc_coin, &apt_coin) {
                    has_btc_pair = true;
                }

                if reserved_conin_id.coin_ids() == (&aptoge_coin, &apt_coin) {
                    has_aptoge_pair = true;
                }
            }
            (has_usdc_pair, has_usdt_pair, has_btc_pair, has_aptoge_pair)
        },
    };

    if !has_usdc_pair {
        (32000_0000_0000, aggregator::LoanCounterCoin::Usdc)
    } else if !has_usdt_pair {
        (2000_0000_0000, aggregator::LoanCounterCoin::Usdt)
    } else if !has_btc_pair {
        (1936_0000_0000, aggregator::LoanCounterCoin::Btc)
    } else if !has_aptoge_pair {
        (1880_0000_0000, aggregator::LoanCounterCoin::Aptoge)
    } else {
        (not_use_loan_max_amount, aggregator::LoanCounterCoin::Usdc)
    }
}

pub async fn search_route<'a>(
    node_id: &str,
    target_version: u64,
    affected_swap_updates: Vec<(SwapContract, UpdatePairType)>,
    swap_router_pool: &SwapRouterPool,
    threshold_profit_amount: ProfitAmout,
    not_use_loan_max_amount: UseLoanAmount,

    ensure_min_out: bool,

    input_amounts: Vec<u64>,
    swap_filters_for_steps: Option<(
        Option<SwapContract>,
        Option<SwapContract>,
        Option<SwapContract>,
    )>,
    coin_filters_for_steps: (
        Vec<RoutePairFilter>,
        Vec<RoutePairFilter>,
        Vec<RoutePairFilter>,
    ),
    exclude_in_all_route_swap: Option<SwapContract>,
) -> Result<Option<(SearchRouteResult, ContractArgs)>> {
    let traverse_start_at = Utc::now();
    let best_route = find_best_route(
        swap_router_pool,
        input_amounts.clone(),
        swap_filters_for_steps.clone(),
        coin_filters_for_steps.clone(),
        exclude_in_all_route_swap,
    )
    .await;
    let traverse_duration = Utc::now() - traverse_start_at;

    match best_route {
        Err(e) => {
            error!("traverse error {e:?}");
            Err(e)
        },
        Ok(best_route) => match best_route {
            None => {
                warn!(
                    "no route found. target_version:{:?}, affected_swap_updates:{:?}, swap_filters_for_steps:{:?}, coin_filters_for_steps:{:?},input_amounts:{:?}",
                    target_version,
                    affected_swap_updates,
                    swap_filters_for_steps,
                    coin_filters_for_steps,
                    input_amounts
                );
                Ok(None)
            },

            Some(best_route) => {
                let optimize_start_at = Utc::now();
                let threshold_profit_amount = threshold_profit_amount.0;
                let not_use_loan_max_amount = not_use_loan_max_amount.0;

                let original_input_amount = best_route.input_amount;
                let original_output_amount = best_route
                    .route_steps
                    .last()
                    .map(|step| step.out_amount)
                    .unwrap_or(0);

                // get loan info
                let (max_input_amount, loan_counter_coin) =
                    loan_info(&best_route, swap_router_pool, not_use_loan_max_amount);

                // Loan fee calculation disabled to avoid partial optimization
                let calc_loan_fee_in_optimization = |_input_amount: u64| -> u64 { 0 };

                let best_route = optimize_route_amount_with_loan_fee(
                    best_route,
                    None,
                    (
                        LINEAR_OPTIMIZE_STEP_RANGE_AMOUNT,
                        LINEAR_OPTIMIZE_STEP_MAX_STEP,
                    ),
                    &swap_router_pool,
                    &calc_loan_fee_in_optimization,
                    Some(max_input_amount),
                )?;
                let optimize_duration = Utc::now() - optimize_start_at;

                let input_amount = best_route.input_amount;
                let (use_loan, loan_fee) = if input_amount > not_use_loan_max_amount {
                    (
                        true,
                        crate::swap::liquid::flashloan::calc_flashloan_fee(input_amount),
                    )
                } else {
                    (false, 0)
                };

                let route_infos = swap_router_pool.convert_swap_route_infos(best_route);
                let output_amount = route_infos.last().unwrap().output_amount;

                let route_logs: Vec<RouteLog> = route_infos
                    .iter()
                    .map(|route_info| RouteLog {
                        contract: route_info.contract.clone(),
                        in_coin: route_info.swap_pair_info.in_coin.as_coin_id().clone(),
                        out_coin: route_info.swap_pair_info.out_coin.as_coin_id().clone(),
                        is_x_to_y: route_info.is_x_to_y,
                        input_amount: route_info.input_amount,
                        output_amount: route_info.output_amount,
                    })
                    .collect();

                let estimated_max_gas = estimate_max_gas(&route_logs, use_loan);

                if output_amount == 0 || output_amount <= input_amount + loan_fee {
                    return Ok(None);
                }

                let gas_unit_price = calc_gas_unit_price(
                    output_amount - input_amount,
                    EstimateGas(estimated_max_gas),
                    &route_infos,
                );

                let estimated_gas_fee = gas_unit_price * estimated_max_gas;

                let return_amount =
                    output_amount - std::cmp::min(output_amount, loan_fee + estimated_gas_fee);

                let profit_amount = return_amount - std::cmp::min(return_amount, input_amount);
                if profit_amount == 0 {
                    debug!(
                        "route candidate with zero profit: version={}, input={}, output={}",
                        target_version, input_amount, output_amount
                    );
                    return Ok(None);
                }

                if threshold_profit_amount < profit_amount && input_amount < MAX_INPUT_APT_AMOUNT {
                    let (contract_func_args, contract_func_types) =
                        aggregator::convert_hippo_args_v2(
                            input_amount,
                            &route_infos,
                            ensure_min_out,
                            loan_counter_coin,
                        );

                    let specified_max_gas_amount = Some(specific_max_gas_amount(estimated_max_gas));

                    Ok(Some((
                        SearchRouteResult {
                            traverse_start_at,
                            traverse_duration_micro: traverse_duration
                                .num_microseconds()
                                .unwrap_or(0),

                            optimize_start_at,
                            optimize_duration_micro: optimize_duration
                                .num_microseconds()
                                .unwrap_or(0),

                            threshold_profit_amount,

                            original_input_amount,
                            original_output_amount,
                            input_amount,

                            estimated_max_gas,
                            estimated_gas_fee,
                            gas_unit_price,
                            use_loan,
                            loan_fee,
                            specified_max_gas_amount,
                            output_amount,
                            profit_amount,
                            route_logs,
                            target_version,
                        },
                        ContractArgs {
                            contract_func_args,
                            contract_func_types,
                        },
                    )))
                } else {
                    debug!(
                        "route candidate below threshold or over max input: version={}, profit={}, threshold={}, input={}",
                        target_version, profit_amount, threshold_profit_amount, input_amount
                    );
                    Ok(None)
                }
            },
        },
    }
}
