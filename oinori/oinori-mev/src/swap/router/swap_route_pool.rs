use aptos_logger::prelude::*;
use crate::coins::CoinId;
use crate::swap::{
    CoinPairIndexInPool, CoinPairPoolOfSwap, CoinSwapDirection, ReservedCoin, ReservedCoinPair,
    ReservedCoinPairAndIndex, ReservedCoinPairWithVersion, RoutePairFilter, RoutePairIterator,
    SwapContract, SwapPairInfo, SwapResource, UpdatePairType,
};
use anyhow::Result;
use aptos_api_types::MoveConverter;

use aptos_storage_interface::state_view::DbStateView;

use crate::hasher::*;
use serde::{Deserialize, Serialize};

use aptos_storage_interface::DbReader;
use std::sync::Arc;

#[derive(Hash, Clone, Eq, PartialEq, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub struct SwapPairIndex {
    pub swap_index: u8,  // use u8 instead of usize
    pub pair_index: u16, // use u16 instead of usize
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct SwapPairIndexWithDirection {
    pub swap_pair_index: SwapPairIndex,
    pub is_x_to_y: bool,
}

impl SwapPairIndexWithDirection {
    pub fn swap_index(&self) -> u8 {
        self.swap_pair_index.swap_index
    }

    pub fn pair_index(&self) -> u16 {
        self.swap_pair_index.pair_index
    }
}

#[derive(Default)]
pub struct SwapRouterPoolOptions {
    pub exclude_swaps: Vec<SwapContract>,
    pub coin_filter: Vec<CoinId>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SwapRouterPool {
    swap_index: HashMap<SwapContract, usize>,
    swaps: Vec<CoinPairPoolOfSwap>,
}

impl Default for SwapRouterPool {
    fn default() -> Self {
        Self {
            swap_index: HashMap::default(),
            swaps: Vec::default(),
        }
    }
}

impl SwapRouterPool {
    pub fn overwrite_with(&mut self, other: SwapRouterPool) {
        self.swap_index = other.swap_index;
        self.swaps = other.swaps;
    }
    pub fn into_coin_pair_pool_of_swaps(self) -> Vec<CoinPairPoolOfSwap> {
        self.swaps
    }

    pub fn swaps_set(&self) -> HashSet<SwapContract> {
        self.swaps.iter().map(|x| x.swap_contract.clone()).collect()
    }

    pub fn update_pool_with_resources(
        &mut self,
        version: u64,
        resources: Vec<SwapResource>,
        allow_update_with_lower_version: bool,
    ) -> Result<Vec<(SwapContract, UpdatePairType)>> {
        let mut update_pair_types = vec![];
        for each_resource in resources.into_iter() {
            if let Some(swap_pool_index) = self.swap_index.get(&each_resource.swap_contract()) {
                let swap_pool = self.swaps.get_mut(*swap_pool_index).unwrap();
                if let Some(reseverd_coin_pair) = each_resource.into_reserverd_coin_pair()? {
                    let swap_contract = swap_pool.swap_contract.clone();
                    if let Some(update_pair_result) = swap_pool.update_swap_pair_pool(
                        version,
                        reseverd_coin_pair,
                        allow_update_with_lower_version,
                    ) {
                        update_pair_types
                            .push((swap_contract, update_pair_result.as_update_pair_type()))
                    }
                }
            }
        }
        Ok(update_pair_types)
    }

    pub fn update_pool_with_coin_pairs(
        &mut self,
        coin_pair_pools: Vec<CoinPairPoolOfSwap>,
        allow_update_with_lower_version: bool,
    ) -> Result<usize> {
        let mut update_pair_num = 0;
        for each_new_pool in coin_pair_pools.into_iter() {
            if let Some(swap_pool_index) = self.swap_index.get(&each_new_pool.swap_contract) {
                let current_swap_pool = self.swaps.get_mut(*swap_pool_index).unwrap();
                for each_new_coin_pair in each_new_pool.into_coin_pairs_with_version() {
                    let ReservedCoinPairWithVersion {
                        version,
                        reserverd_coin_pair,
                    } = each_new_coin_pair;

                    if current_swap_pool
                        .update_swap_pair_pool(
                            version,
                            reserverd_coin_pair,
                            allow_update_with_lower_version,
                        )
                        .is_some()
                    {
                        update_pair_num += 1;
                    }
                }
            }
        }
        Ok(update_pair_num)
    }

    pub fn len(&self) -> usize {
        self.swaps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.swaps.is_empty()
    }

    pub unsafe fn get_by_index_unchecked(&self, index: usize) -> &CoinPairPoolOfSwap {
        self.swaps.get_unchecked(index)
    }

    pub fn get_routes_from<'a, 'b>(
        &'a self,
        input_coin: CoinId,
        filter_swap_index: Option<usize>,
        exclude_swap_index: Option<usize>,
        coin_pair_filters: Option<&'b [RoutePairFilter]>,
    ) -> SwapRouteIterator<'a, 'b>
    where
        'a: 'b,
    {
        SwapRouteIterator::new(
            self,
            input_coin,
            filter_swap_index,
            exclude_swap_index,
            coin_pair_filters,
        )
    }

    pub unsafe fn get_pair_by_swap_index_unchecked(
        &self,
        index: &SwapPairIndex,
    ) -> &ReservedCoinPair {
        self.get_by_index_unchecked(index.swap_index as usize)
            .get_by_index_unchecked(&CoinPairIndexInPool(index.pair_index as usize))
    }

    pub fn load_from_db(
        db_reader: Arc<dyn DbReader>,
        target_version: Option<u64>,
        move_converter: &MoveConverter<DbStateView>,
        option: Option<SwapRouterPoolOptions>,
    ) -> Result<Self> {
        let option = option.unwrap_or_default();

        let exclude_swaps: HashSet<SwapContract> = option.exclude_swaps.into_iter().collect();
        let target_swaps = SwapContract::all()
            .into_iter()
            .filter(|each_swap_contract| {
                if exclude_swaps.is_empty() {
                    true
                } else {
                    !exclude_swaps.contains(each_swap_contract)
                }
            })
            .collect::<Vec<SwapContract>>();
        info!("loading swaps target_swaps ");

        let mut swap_pools = vec![];
        let coin_filter = option.coin_filter;
        for each_swap in target_swaps {
            let coin_pool = CoinPairPoolOfSwap::from_db(
                each_swap,
                db_reader.clone(),
                target_version,
                move_converter,
                coin_filter.clone(),
            )?;
            swap_pools.push(coin_pool);
        }
        let swap_index = swap_pools
            .iter()
            .enumerate()
            .map(|(idx, pool)| (pool.swap_contract.clone(), idx))
            .collect();
        Ok(Self {
            swap_index,
            swaps: swap_pools,
        })
    }

    pub fn find_swap(&self, swap_contract: &SwapContract) -> Option<&CoinPairPoolOfSwap> {
        self.swaps
            .iter()
            .find(|swap| &swap.swap_contract == swap_contract)
    }

    pub fn get_swap_by_index(&self, index: usize) -> Option<&CoinPairPoolOfSwap> {
        self.swaps.get(index)
    }

    pub unsafe fn get_swap_by_index_unchecked(&self, index: usize) -> &CoinPairPoolOfSwap {
        self.swaps.get_unchecked(index)
    }

    pub fn get_swap_index(&self, swap_contact: &SwapContract) -> Option<u8> {
        self.swap_index.get(swap_contact).map(|index| *index as u8)
    }

    pub fn convert_swap_route_infos<'a>(
        &'a self,
        swap_route: SwapRoute,
    ) -> Vec<SwapRouteStepInfo<'a>> {
        let mut each_input_amount: u64 = swap_route.input_amount;
        let mut result_swap_route_infos = Vec::with_capacity(swap_route.len());
        for each_step in swap_route.route_steps.into_iter() {
            let SwapRouteStep {
                swap_index_and_direction: route_indices,
                out_amount,
            } = each_step;

            let SwapPairIndexWithDirection {
                swap_pair_index,
                is_x_to_y,
            } = route_indices;

            let (swap, coin_pair) = unsafe {
                let swap = self.get_swap_by_index_unchecked(swap_pair_index.swap_index as usize);
                let coin_pair = swap.get_by_index_unchecked(&CoinPairIndexInPool(
                    swap_pair_index.pair_index as usize,
                ));
                (swap, coin_pair)
            };

            let swap_direction = if is_x_to_y {
                CoinSwapDirection::XToY
            } else {
                CoinSwapDirection::YToX
            };
            let swap_pair_info = coin_pair.as_swap_pair_info(swap_direction);
            let input_amount = each_input_amount;
            let each_swap_route_step_info = SwapRouteStepInfo {
                contract: &swap.swap_contract,
                swap_pair_info,
                is_x_to_y,
                input_amount,
                output_amount: out_amount,
            };
            result_swap_route_infos.push(each_swap_route_step_info);
            each_input_amount = out_amount;
        }

        result_swap_route_infos
    }
}

pub struct SwapContractIndex(usize);

#[derive(Default)]
struct SwapRouteIteratorStatus<'a, 'b> {
    current_swap_contract_index: usize,
    current_swap_read_y_to_x_prior: bool,
    current_pair_iterator: Option<RoutePairIterator<'a, 'b>>,
}

impl<'a, 'b> SwapRouteIteratorStatus<'a, 'b>
where
    'a: 'b,
{
    fn proceed_to_next_pair_iterator(
        &mut self,
        pool: &'a SwapRouterPool,
        input_coin: &CoinId,
        filter_swap_index: &Option<usize>,
        coin_pair_filters: Option<&'b [RoutePairFilter]>,
        exclude_swap_index: &Option<usize>,
    ) -> bool {
        //TODO (tacogips) performance bottle neck? << yes, it might be
        //if let (Some(filter_swap_index_to_check), Some(exclude_swap_index_to_check)) =
        //    (filter_swap_index.as_ref(), exclude_swap_index.as_ref())
        //{
        //    // its invalid condition
        //    if *filter_swap_index_to_check == *exclude_swap_index_to_check {
        //        return false;
        //    }
        //}

        let has_initialized = self.current_pair_iterator.is_some();

        if has_initialized && !self.current_swap_read_y_to_x_prior {
            self.current_swap_contract_index += 1;
        };

        if let Some(filter_swap_index) = filter_swap_index {
            if !has_initialized {
                self.current_swap_contract_index = *filter_swap_index;
            }
        }

        // do this wihtout the loop  below once for async performance
        if let Some(exclude_swap_index) = exclude_swap_index {
            if self.current_swap_contract_index == *exclude_swap_index {
                self.current_swap_contract_index += 1;
            }
        }

        loop {
            if let Some(filter_swap_index) = filter_swap_index {
                if self.current_swap_contract_index != *filter_swap_index {
                    return false; // end of all swaps
                }
            }

            if let Some(exclude_swap_index) = exclude_swap_index {
                if self.current_swap_contract_index == *exclude_swap_index {
                    self.current_swap_contract_index += 1;
                }
            }
            match pool.get_swap_by_index(self.current_swap_contract_index) {
                Some(current_swap) => {
                    //TODO() perfomance tuning . dont use coin_exists() here

                    if !self.current_swap_read_y_to_x_prior {
                        if let Some(x_to_y_route_iter) = current_swap
                            .coin_pair_pool
                            .get_routes_by_input_x(input_coin, coin_pair_filters)
                        {
                            self.current_pair_iterator.replace(x_to_y_route_iter);
                            self.current_swap_read_y_to_x_prior = true;

                            return true;
                        }
                    }

                    if let Some(y_to_x_route_iter) = current_swap
                        .coin_pair_pool
                        .get_routes_by_input_y(input_coin, coin_pair_filters)
                    {
                        self.current_pair_iterator.replace(y_to_x_route_iter);
                        self.current_swap_read_y_to_x_prior = false;
                        return true;
                    } else {
                        // go to next swap if the coin exists in the current swap
                        self.current_swap_contract_index += 1;
                        self.current_swap_read_y_to_x_prior = false;
                        continue;
                    }
                },
                None => return false, // end of all swaps
            }
        }
    }
}

pub struct SwapRouteIterator<'a, 'b> {
    pool: &'a SwapRouterPool,
    input_coin: CoinId,
    status: SwapRouteIteratorStatus<'a, 'b>,
    filter_swap_index: Option<usize>,
    exclude_swap_index: Option<usize>,
    filter_coin_pairs: Option<&'b [RoutePairFilter]>,
}

impl<'a, 'b> SwapRouteIterator<'a, 'b> {
    pub fn new(
        pool: &'a SwapRouterPool,
        input_coin: CoinId,
        filter_swap_index: Option<usize>,
        exclude_swap_index: Option<usize>,
        filter_coin_pairs: Option<&'b [RoutePairFilter]>,
    ) -> Self
    where
        'a: 'b,
    {
        Self {
            pool,
            input_coin,
            status: SwapRouteIteratorStatus::default(),
            filter_swap_index,
            exclude_swap_index,
            filter_coin_pairs,
        }
    }
}

#[derive(Debug)]
pub struct ReservedCoinPairAndIndexOfSwap<'a> {
    pub swap_index: usize,
    pub reserved_pair_and_index: ReservedCoinPairAndIndex<'a>,
}

impl<'a> ReservedCoinPairAndIndexOfSwap<'a> {
    pub fn is_stable_coin_pair(&self) -> bool {
        self.reserved_pair_and_index.is_stable_coin_pair()
    }
    pub fn input_coin_id(&self) -> &CoinId {
        &self.reserved_pair_and_index.get_input_coin().coin.coin_id
    }

    pub fn output_coin_id(&self) -> &CoinId {
        &self.reserved_pair_and_index.get_output_coin().coin.coin_id
    }

    pub fn calc_amount_out(&self, in_amount: u64) -> Result<u64> {
        self.reserved_pair_and_index.calc_amount_out(in_amount)
    }

    pub fn get_input_coin(&self) -> &'a ReservedCoin {
        self.reserved_pair_and_index.get_input_coin()
    }

    pub fn get_output_coin(&self) -> &'a ReservedCoin {
        self.reserved_pair_and_index.get_output_coin()
    }

    pub fn as_swap_pair_index(&self) -> SwapPairIndexWithDirection {
        SwapPairIndexWithDirection {
            swap_pair_index: SwapPairIndex {
                swap_index: self.swap_index as u8,
                pair_index: self.reserved_pair_and_index.pair_index_in_pool.0 as u16,
            },
            is_x_to_y: self.reserved_pair_and_index.direction == CoinSwapDirection::XToY,
        }
    }
}

impl<'a, 'b> Iterator for SwapRouteIterator<'a, 'b>
where
    'a: 'b,
{
    type Item = ReservedCoinPairAndIndexOfSwap<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current_pair_iterator) = self.status.current_pair_iterator.as_mut() {
            let next_pair = current_pair_iterator.next();

            if let Some(next_pair) = next_pair {
                if !next_pair.has_empty_reserved_amount() {
                    return Some(ReservedCoinPairAndIndexOfSwap {
                        swap_index: self.status.current_swap_contract_index,
                        reserved_pair_and_index: next_pair,
                    });
                }
            }
        }

        if self.status.proceed_to_next_pair_iterator(
            self.pool,
            &self.input_coin,
            &self.filter_swap_index,
            self.filter_coin_pairs,
            &self.exclude_swap_index,
        ) {
            self.next()
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

#[derive(Debug)]
pub struct CoinPairAndSwapDirection<'a> {
    pub reserved_coin_pair: &'a ReservedCoinPair,
    pub direction: CoinSwapDirection,
}
impl<'a> CoinPairAndSwapDirection<'a> {
    pub fn calc_amount_out(&self, input_amount: u64) -> Result<u64> {
        self.reserved_coin_pair
            .calc_amount_out(input_amount, &self.direction)
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SwapRoute {
    pub input_amount: u64,
    pub route_steps: Vec<SwapRouteStep>,
}

impl SwapRoute {
    pub fn new(input_amount: u64, route_steps: Vec<SwapRouteStep>) -> Self {
        debug_assert!(!route_steps.is_empty());
        Self {
            input_amount,
            route_steps,
        }
    }

    pub fn contains_contract_index(&self, contract_index: u8) -> bool {
        self.route_steps
            .iter()
            .any(|step| step.swap_index_and_direction.swap_index() == contract_index)
    }

    pub fn find_by_swap_index(&self, contract_index: u8) -> Vec<&SwapRouteStep> {
        self.route_steps
            .iter()
            .filter(|step| step.swap_index_and_direction.swap_index() == contract_index)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.route_steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.route_steps.is_empty()
    }

    pub fn last_output_amount(&self) -> u64 {
        self.route_steps.last().unwrap().out_amount
    }
    pub fn profit(&self) -> u64 {
        let last_amount = self.last_output_amount();
        if last_amount > self.input_amount {
            last_amount - self.input_amount
        } else {
            0
        }
    }

    pub fn has_bigger_profit_than(&self, other: &SwapRoute) -> bool {
        self.profit() > other.profit()
    }

    pub fn as_coin_pairs_and_directions<'a>(
        &self,
        router_pool: &'a SwapRouterPool,
    ) -> Vec<CoinPairAndSwapDirection<'a>> {
        self.route_steps
            .iter()
            .map(|step| {
                let reserved_coin_pair = unsafe {
                    router_pool.get_pair_by_swap_index_unchecked(
                        &step.swap_index_and_direction.swap_pair_index,
                    )
                };
                let direction = if step.swap_index_and_direction.is_x_to_y {
                    CoinSwapDirection::XToY
                } else {
                    CoinSwapDirection::YToX
                };

                CoinPairAndSwapDirection {
                    reserved_coin_pair,
                    direction,
                }
            })
            .collect()
    }

    pub fn as_step_output_amounts<'a>(&self) -> Vec<u64> {
        self.route_steps
            .iter()
            .map(|step| step.out_amount)
            .collect()
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SwapRouteStep {
    pub swap_index_and_direction: SwapPairIndexWithDirection,
    pub out_amount: u64,
}
impl SwapRouteStep {
    pub fn swap_index(&self) -> u8 {
        self.swap_index_and_direction.swap_index()
    }

    pub fn coin_pair_index(&self) -> u16 {
        self.swap_index_and_direction.pair_index()
    }
}

#[derive(Debug, Serialize)]
pub struct SwapRouteStepInfo<'a> {
    pub contract: &'a SwapContract,
    pub swap_pair_info: SwapPairInfo<'a>,
    pub is_x_to_y: bool,
    // TODO add pool type
    pub input_amount: u64,
    pub output_amount: u64,
}
