use aptos_logger::prelude::*;
use crate::coins::{apt_coin_id, CoinId};

use crate::hasher::*;
use crate::swap::SwapContract;
use crate::swap::{
    ReservedCoinPairAndIndexOfSwap, RoutePairFilter, SwapPairIndex, SwapRoute, SwapRouteStep,
    SwapRouterPool,
};
use anyhow::Result;
use async_recursion::async_recursion;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::time::{sleep, Duration};

const DEFAULT_TIMEOUT_MICRO_SEC: u64 = 50_000; //50ms

pub async fn start_route_traverse(
    router_pool: &SwapRouterPool,
    input_coin_id: CoinId,
    input_amounts: Vec<u64>,
    timeout: Option<Duration>,
    max_depth: Option<u8>,
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
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<(u64, Vec<SwapRouteStep>)>>();

    let router_swap_len = router_pool.len();

    // if route which we will search contains xxx-apt pair, the route must be start the pair
    let (
        first_step_contract_filter_index,
        second_step_swap_filter_index,
        third_step_swap_filter_index,
    ) = match swap_filters_for_steps {
        None => (None, None, None),
        Some(swap_contract_filter) => {
            let (first_swap_filter, second_swap_filter, third_swap_filter) = swap_contract_filter;
            let start_filter_swap_index = first_swap_filter.as_ref().map(|swap| {
                router_pool.get_swap_index(&swap).expect(&format!(
                    "error in traverse. first filtering swap {:?} must be exists in the pool",
                    first_swap_filter
                ))
            });

            let second_filter_swap_index = second_swap_filter.as_ref().map(|swap| {
                router_pool.get_swap_index(&swap).expect(&format!(
                    "error in travese. second filtering swap {:?} must be exists in the pool",
                    second_swap_filter
                ))
            });

            let third_filter_swap_index = third_swap_filter.as_ref().map(|swap| {
                router_pool.get_swap_index(&swap).expect(&format!(
                    "error in travese. third filtering swap {:?} must be exists in the pool",
                    second_swap_filter
                ))
            });

            (
                start_filter_swap_index.map(|v| v as usize),
                second_filter_swap_index.map(|v| v as usize),
                third_filter_swap_index.map(|v| v as usize),
            )
        },
    };

    let exclude_in_all_route_swap_filter_index = {
        exclude_in_all_route_swap
            .as_ref()
            .map(|swap| router_pool.get_swap_index(&swap))
            .flatten()
            .map(|v| v as usize)
    };

    // start sink
    let router_finding_handler = tokio::task::spawn(async move {
        let timeout = sleep(timeout.unwrap_or(Duration::from_micros(DEFAULT_TIMEOUT_MICRO_SEC)));
        tokio::pin!(timeout);
        let mut best_route: Option<SwapRoute> = None;

        loop {
            tokio::select! {
                _ = &mut timeout =>{
                   info!("route finding timeout.reached");
                   break
                },

                new_routes = rx.recv() =>{
                    match new_routes {
                        None=> break,
                        Some(new_routes)=>{
                            for (input_amount, new_route_steps)  in new_routes.into_iter(){
                                let new_route = SwapRoute::new(input_amount, new_route_steps);

                                match best_route.as_ref(){
                                    None => {best_route.replace(new_route);},
                                    Some(best_route_val)=> if new_route.has_bigger_profit_than(best_route_val){
                                        best_route.replace(new_route);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        best_route
    });

    let start_step_coin_filters = coin_filters_for_steps.0.clone();
    let start_step_coin_filter = if start_step_coin_filters.is_empty() {
        None
    } else {
        Some(start_step_coin_filters)
    };

    for each_input_amount in input_amounts {
        for each_router_start_swap_index in 0..router_swap_len {
            // skip if stricted
            if let Some(start_step_contract_index) = first_step_contract_filter_index {
                if each_router_start_swap_index != start_step_contract_index {
                    continue;
                }
            }

            let cloned_input_coin_id = input_coin_id.clone();
            let each_destination_coin = input_coin_id.clone();

            // !!! deep copy here !!!!
            let cloned_shared_router_pool = router_pool.clone();

            let cloned_destination_sender = tx.clone();
            let cloned_start_step_coin_filter = start_step_coin_filter.clone();
            let cloned_coin_filters_for_steps = coin_filters_for_steps.clone();
            tokio::task::spawn(async move {
                let first_step_routes = cloned_shared_router_pool.get_routes_from(
                    cloned_input_coin_id,
                    Some(each_router_start_swap_index),
                    exclude_in_all_route_swap_filter_index,
                    cloned_start_step_coin_filter.as_ref().map(|v| v.as_slice()),
                );

                let (_, second_step_coin_filter, third_step_coin_filter) =
                    cloned_coin_filters_for_steps;

                let swap_filters_for_each_steps = vec![
                    None,
                    second_step_swap_filter_index,
                    third_step_swap_filter_index,
                ];

                let coin_filters_for_each_steps =
                    vec![vec![], second_step_coin_filter, third_step_coin_filter];

                traverse(
                    &each_input_amount,
                    &cloned_shared_router_pool,
                    first_step_routes,
                    each_input_amount,
                    &each_destination_coin,
                    RouteManager::new(max_depth),
                    swap_filters_for_each_steps.as_slice(),
                    coin_filters_for_each_steps.as_slice(),
                    exclude_in_all_route_swap_filter_index,
                    cloned_destination_sender,
                )
                .await
            });
        }
    }
    drop(tx); // drop unused tx

    let result = router_finding_handler.await?;
    Ok(result)
}

#[async_recursion]
pub async fn traverse<'a, T>(
    first_input_amount: &u64,
    router_pool: &SwapRouterPool,
    input_route_steps: T,
    input_amount: u64,
    destination_coin: &CoinId,
    route_manager: RouteManager,
    route_swap_filter_index: &[Option<usize>],
    coin_filters_for_each_stepas: &[Vec<RoutePairFilter>],

    exclude_swap_index: Option<usize>,
    destination_sender: UnboundedSender<Vec<(u64, Vec<SwapRouteStep>)>>,
) where
    T: Iterator<Item = ReservedCoinPairAndIndexOfSwap<'a>> + Send,
{
    if input_amount == 0 {
        return;
    }
    let mut result_routes: Vec<(u64, Vec<SwapRouteStep>)> = Vec::new();

    let next_step_index = (route_manager.current_depth + 1) as usize;

    let next_routes_swap_filter_index = route_swap_filter_index
        .get(next_step_index)
        .cloned()
        .flatten();

    let coin_filter_for_next_step = coin_filters_for_each_stepas.get((next_step_index) as usize);

    for each_route_step in input_route_steps {
        let each_swap_pair_index = each_route_step.as_swap_pair_index();

        if !route_manager.already_visited(&each_swap_pair_index.swap_pair_index) {
            match each_route_step.calc_amount_out(input_amount) {
                Err(e) => {
                    error!("calc amount error {e:?},{:?} ", route_manager);
                },
                Ok(out_amount) => {
                    if out_amount == 0 {
                        continue;
                    }

                    if &each_route_step.get_output_coin().coin.coin_id == destination_coin {
                        let mut routes_to_destination = route_manager.routes.clone();
                        routes_to_destination.push(SwapRouteStep {
                            swap_index_and_direction: each_swap_pair_index,
                            out_amount,
                        });

                        result_routes.push((*first_input_amount, routes_to_destination));
                    } else if !route_manager.next_depth_reaches_to_max() {
                        let next_routes = router_pool.get_routes_from(
                            each_route_step.output_coin_id().clone(),
                            next_routes_swap_filter_index,
                            exclude_swap_index,
                            coin_filter_for_next_step.map(|v| v.as_slice()),
                        );

                        traverse(
                            &first_input_amount,
                            router_pool,
                            next_routes,
                            out_amount,
                            destination_coin,
                            route_manager.add_route_step_clone(SwapRouteStep {
                                swap_index_and_direction: each_swap_pair_index,
                                out_amount,
                            }),
                            route_swap_filter_index.clone(), // swap filter for depth more then third is not available
                            coin_filters_for_each_stepas,
                            exclude_swap_index,
                            destination_sender.clone(),
                        )
                        .await
                    }
                },
            }
        }
    }

    let _ = destination_sender.send(result_routes);
}

#[derive(Clone, Default, Debug)]
pub struct RouteManager {
    visited_routes: HashSet<SwapPairIndex>,
    routes: Vec<SwapRouteStep>,
    current_depth: u8,
    max_depth: u8,
}

impl RouteManager {
    fn new(max_depth: Option<u8>) -> Self {
        Self {
            max_depth: max_depth.unwrap_or(3u8),
            routes: Vec::with_capacity(max_depth.unwrap_or(3u8) as usize),
            ..Default::default()
        }
    }

    fn already_visited(&self, swap_and_index: &SwapPairIndex) -> bool {
        self.visited_routes.contains(swap_and_index)
    }

    fn next_depth_reaches_to_max(&self) -> bool {
        self.current_depth + 1 >= self.max_depth
    }

    fn add_route_step_clone(&self, swap_route: SwapRouteStep) -> Self {
        let mut visited_routes = self.visited_routes.clone();
        visited_routes.insert(swap_route.swap_index_and_direction.swap_pair_index.clone());
        let mut routes = self.routes.clone();
        routes.push(swap_route);

        Self {
            visited_routes,
            routes,
            current_depth: self.current_depth + 1,
            max_depth: self.max_depth,
        }
    }
}
