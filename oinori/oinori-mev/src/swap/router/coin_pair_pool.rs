use crate::coins::{Coin, CoinId};
use crate::read_db;
use crate::swap;
use crate::swap::amm::{CoinSwapDirection, ReservedCoin};

use crate::hasher::*;
use crate::swap::{ReservedCoinPair, SwapContract};
use anyhow::Result;
use aptos_api_types::MoveConverter;
use aptos_storage_interface::DbReader;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use aptos_storage_interface::state_view::DbStateView;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoinPairPoolOfSwap {
    pub swap_contract: SwapContract,
    pub coin_pair_pool: CoinPairPool,
    pub coin_filters: HashSet<CoinId>,
}

impl CoinPairPoolOfSwap {
    pub fn update_swap_pair_pool<'a>(
        &'a mut self,
        version: u64,
        coin_pair: ReservedCoinPair,
        allow_updated_by_lower_version: bool,
    ) -> Option<UpdatePairResult<'a>> {
        if !self.coin_filters.is_empty() {
            let (x_coin, y_coin) = coin_pair.coin_ids();
            if !self.coin_filters.contains(x_coin) || !self.coin_filters.contains(y_coin) {
                return None;
            }
        }

        self.coin_pair_pool
            .update_or_insert(version, coin_pair, allow_updated_by_lower_version)
    }
    pub fn into_coin_pairs_with_version(self) -> Vec<ReservedCoinPairWithVersion> {
        self.coin_pair_pool.into_paris_with_vesion()
    }

    pub unsafe fn get_by_index_unchecked(&self, index: &CoinPairIndexInPool) -> &ReservedCoinPair {
        self.coin_pair_pool.get_by_index_unchecked(index)
    }

    pub(crate) fn from_db(
        swap_contract: SwapContract,
        db_reader: Arc<dyn DbReader>,
        target_version: Option<u64>,
        move_converter: &MoveConverter<DbStateView>,
        coin_filters: Vec<CoinId>,
    ) -> Result<Self> {
        let pool_address = swap_contract.pool_address();
        let resources_and_version = read_db::get_resources_of_account(
            db_reader,
            pool_address,
            target_version,
            move_converter,
        )?;

        let coin_filters: HashSet<CoinId> = coin_filters.into_iter().collect();

        let resources = swap::SwapResource::from_resources(&resources_and_version.resources)?;

        let coin_pairs = resources
            .into_iter()
            .filter_map(|resource| match resource.into_reserverd_coin_pair() {
                Ok(reserved_coin) => match reserved_coin {
                    Some(reserved_coin) => {
                        if reserved_coin.has_empty_reserved_amount() {
                            None
                        } else if coin_filters.is_empty() {
                            Some(reserved_coin)
                        } else {
                            let (x_coin, y_coin) = reserved_coin.coin_ids();

                            if coin_filters.contains(x_coin) && coin_filters.contains(y_coin) {
                                Some(reserved_coin)
                            } else {
                                None
                            }
                        }
                    },

                    None => None,
                },
                Err(_) => None,
            })
            .collect();

        //if y_coin.coin
        Ok(Self {
            swap_contract,
            coin_pair_pool: CoinPairPool::from_reservered_coins(
                resources_and_version.version,
                coin_pairs,
            ),
            coin_filters,
        })
    }
}

macro_rules! add_pair_indices {
    ($pair_index:expr, $x_coin_id:expr, $y_coin_id:expr, $x_to_y_indices :expr, $y_to_x_indices :expr) => {{
        {
            let y_indice = $x_to_y_indices
                .entry($x_coin_id.clone())
                .or_insert(HashMap::<CoinId, usize>::default());
            y_indice.insert($y_coin_id.clone(), $pair_index);
        }

        {
            let x_indice = $y_to_x_indices
                .entry($y_coin_id)
                .or_insert(HashMap::<CoinId, usize>::default());
            x_indice.insert($x_coin_id, $pair_index);
        }
    }};
}

macro_rules! get_route_pair_iter_by_input {
    ($self:expr,$input_coin_id:expr,$indices:expr,$direction:expr, $coin_pair_filters: expr
     ) => {{
        match $indices.get($input_coin_id) {
            Some(found_reseved_coin_indices) => {
                let pair_indices = found_reseved_coin_indices.values();

                Some(RoutePairIterator {
                    pair_indices,
                    direction: $direction,
                    all_pairs: &$self.pairs_with_version,
                    coin_pair_filters: $coin_pair_filters,
                })
            },
            None => None,
        }
    }};
}

pub enum FoundRoute<'a> {
    XToY(&'a ReservedCoinPair),
    YToX(&'a ReservedCoinPair),
}

pub enum PairRoute<'a> {
    XToY(&'a Coin, &'a Coin),
    YToX(&'a Coin, &'a Coin),
}

impl<'a> PairRoute<'a> {
    pub fn x_to_y(x: &'a Coin, y: &'a Coin) -> Self {
        Self::XToY(x, y)
    }

    pub fn y_to_x(y: &'a Coin, x: &'a Coin) -> Self {
        Self::YToX(x, y)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum UpdatePairType {
    NewPair(CoinId, CoinId),

    XIncreaseYDecrease(CoinId, CoinId),
    XDecreaseYIncrease(CoinId, CoinId),

    XIncreaseYIncrease((CoinId, u64), (CoinId, u64)),
    XDecreaseYDecrease((CoinId, u64), (CoinId, u64)),

    Other(CoinId, CoinId),
}

pub struct UpdatePairResult<'a> {
    pub old: Option<ReservedCoinPairWithVersion>,
    pub new: &'a ReservedCoinPairWithVersion,
}
impl<'a> UpdatePairResult<'a> {
    pub fn as_update_pair_type(&self) -> UpdatePairType {
        match &self.old {
            None => UpdatePairType::NewPair(
                self.new.coin_x().as_coin_id().clone(),
                self.new.coin_y().as_coin_id().clone(),
            ),
            Some(old) => {
                let old_x = old.coin_x();
                let old_y = old.coin_y();

                let new_x = self.new.coin_x();
                let new_y = self.new.coin_y();

                if old_x.reserverd_amount > new_x.reserverd_amount
                    && old_y.reserverd_amount < new_y.reserverd_amount
                {
                    // x :decreease
                    // y :increase
                    UpdatePairType::XDecreaseYIncrease(
                        old_x.as_coin_id().clone(),
                        old_y.as_coin_id().clone(),
                    )
                } else if old_x.reserverd_amount < new_x.reserverd_amount
                    && old_y.reserverd_amount > new_y.reserverd_amount
                {
                    // x :increase
                    // y :decrease
                    UpdatePairType::XIncreaseYDecrease(
                        old_x.as_coin_id().clone(),
                        old_y.as_coin_id().clone(),
                    )
                } else if old_x.reserverd_amount < new_x.reserverd_amount
                    && old_y.reserverd_amount < new_y.reserverd_amount
                {
                    UpdatePairType::XIncreaseYIncrease(
                        (
                            old_x.as_coin_id().clone(),
                            new_x.reserverd_amount - old_x.reserverd_amount,
                        ),
                        (
                            old_y.as_coin_id().clone(),
                            new_y.reserverd_amount - old_y.reserverd_amount,
                        ),
                    )
                } else if old_x.reserverd_amount > new_x.reserverd_amount
                    && old_y.reserverd_amount > new_y.reserverd_amount
                {
                    UpdatePairType::XDecreaseYDecrease(
                        (
                            old_x.as_coin_id().clone(),
                            old_x.reserverd_amount - new_x.reserverd_amount,
                        ),
                        (
                            old_y.as_coin_id().clone(),
                            old_y.reserverd_amount - new_y.reserverd_amount,
                        ),
                    )
                } else {
                    UpdatePairType::Other(old_x.as_coin_id().clone(), old_y.as_coin_id().clone())
                }
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReservedCoinPairWithVersion {
    pub version: u64,
    pub reserverd_coin_pair: ReservedCoinPair,
}

impl ReservedCoinPairWithVersion {
    fn coin_x(&self) -> &ReservedCoin {
        &self.reserverd_coin_pair.x
    }

    fn coin_y(&self) -> &ReservedCoin {
        &self.reserverd_coin_pair.y
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoinPairPool {
    pairs_with_version: Vec<ReservedCoinPairWithVersion>,

    x_to_y_indices: HashMap<CoinId, HashMap<CoinId, usize>>,
    y_to_x_indices: HashMap<CoinId, HashMap<CoinId, usize>>,
    all_coin_ids: HashSet<CoinId>,
}

impl CoinPairPool {
    pub fn into_paris_with_vesion(self) -> Vec<ReservedCoinPairWithVersion> {
        self.pairs_with_version
    }

    pub fn from_reservered_coins(version: u64, coin_pairs: Vec<ReservedCoinPair>) -> Self {
        let mut x_to_y_indices = HashMap::<CoinId, HashMap<CoinId, usize>>::default();
        let mut y_to_x_indices = HashMap::<CoinId, HashMap<CoinId, usize>>::default();
        let mut all_coin_ids = HashSet::<CoinId>::default();

        for (pair_index, each_coin_pair) in
            coin_pairs.iter().filter(|pair| pair.available).enumerate()
        {
            let x_coin_id = each_coin_pair.x.coin.as_coin_id().clone();
            let y_coin_id = each_coin_pair.y.coin.as_coin_id().clone();

            all_coin_ids.insert(x_coin_id.clone());
            all_coin_ids.insert(y_coin_id.clone());
            add_pair_indices!(
                pair_index,
                x_coin_id,
                y_coin_id,
                x_to_y_indices,
                y_to_x_indices
            );
        }

        Self {
            pairs_with_version: coin_pairs
                .into_iter()
                .map(|reserved_coin_pair| ReservedCoinPairWithVersion {
                    version,
                    reserverd_coin_pair: reserved_coin_pair,
                })
                .collect(),
            x_to_y_indices,
            y_to_x_indices,
            all_coin_ids,
        }
    }

    fn update_or_insert<'a>(
        &'a mut self,
        version: u64,
        coin_pair: ReservedCoinPair,
        allow_updated_by_lower_version: bool,
    ) -> Option<UpdatePairResult<'a>> {
        let found_route_index = self
            .find_route_index(PairRoute::x_to_y(&coin_pair.x.coin, &coin_pair.y.coin))
            .copied();

        match found_route_index {
            Some(pair_index) => {
                let existing_coin = self.pairs_with_version.get_mut(pair_index).unwrap();

                if !allow_updated_by_lower_version && existing_coin.version > version {
                    return None;
                }
                let mut coin_pair_with_version = ReservedCoinPairWithVersion {
                    version,
                    reserverd_coin_pair: coin_pair,
                };

                //self.pairs.swap();
                std::mem::swap(existing_coin, &mut coin_pair_with_version);

                // reckoning old and new has swaped
                Some(UpdatePairResult {
                    old: Some(coin_pair_with_version),
                    new: existing_coin,
                })
            },

            None => {
                let x_coin_id = coin_pair.x.coin.as_coin_id().clone();
                let y_coin_id = coin_pair.y.coin.as_coin_id().clone();

                self.pairs_with_version.push(ReservedCoinPairWithVersion {
                    version,
                    reserverd_coin_pair: coin_pair,
                });
                add_pair_indices!(
                    self.pairs_with_version.len() - 1,
                    x_coin_id,
                    y_coin_id,
                    self.x_to_y_indices,
                    self.y_to_x_indices
                );

                Some(UpdatePairResult {
                    old: None,
                    new: self.pairs_with_version.last().unwrap(),
                })
            },
        }
    }

    #[deprecated]
    pub fn coin_exists(&self, coin_id: &CoinId) -> bool {
        self.all_coin_ids.contains(coin_id)
    }

    pub fn route_exists(&self, pair_route: PairRoute<'_>) -> bool {
        self.find_route_index(pair_route).is_some()
    }

    pub fn find_route_index(&self, pair_route: PairRoute<'_>) -> Option<&usize> {
        let (first_coin, second_coin, indices) = match pair_route {
            PairRoute::XToY(x, y) => (x, y, &self.x_to_y_indices),
            PairRoute::YToX(y, x) => (y, x, &self.y_to_x_indices),
        };

        if let Some(routes) = indices.get(first_coin.as_coin_id()) {
            routes.get(second_coin.as_coin_id())
        } else {
            None
        }
    }

    pub fn get_routes_by_input_x<'a, 'b>(
        &'a self,
        input_coin_id: &CoinId,
        coin_pair_filters: Option<&'b [RoutePairFilter]>,
    ) -> Option<RoutePairIterator<'a, 'b>>
    where
        'a: 'b,
    {
        get_route_pair_iter_by_input!(
            self,
            input_coin_id,
            &self.x_to_y_indices,
            CoinSwapDirection::XToY,
            coin_pair_filters
        )
    }

    pub fn get_routes_by_input_y<'a, 'b>(
        &'a self,
        input_coin_id: &CoinId,
        coin_pair_filters: Option<&'b [RoutePairFilter]>,
    ) -> Option<RoutePairIterator<'a, 'b>>
    where
        'a: 'b,
    {
        get_route_pair_iter_by_input!(
            self,
            input_coin_id,
            &self.y_to_x_indices,
            CoinSwapDirection::YToX,
            coin_pair_filters
        )
    }

    pub fn get_by_index(&self, index: &CoinPairIndexInPool) -> Option<&ReservedCoinPair> {
        self.pairs_with_version
            .get(index.0)
            .map(|pair| &pair.reserverd_coin_pair)
    }

    pub unsafe fn get_by_index_unchecked(&self, index: &CoinPairIndexInPool) -> &ReservedCoinPair {
        &self
            .pairs_with_version
            .get_unchecked(index.0)
            .reserverd_coin_pair
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePairFilter {
    pub input_coin: Option<CoinId>,
    pub output_coin: Option<CoinId>,
}

pub struct RoutePairIterator<'a, 'b> {
    pair_indices: HashMapValues<'a, CoinId, usize>,
    direction: CoinSwapDirection,
    all_pairs: &'a Vec<ReservedCoinPairWithVersion>,
    pub coin_pair_filters: Option<&'b [RoutePairFilter]>,
}

#[derive(Debug)]
pub struct CoinPairIndexInPool(pub usize); // pair index in a pool

#[derive(Debug)]
pub struct ReservedCoinPairAndIndex<'a> {
    pub pair: &'a ReservedCoinPair,
    pub pair_index_in_pool: CoinPairIndexInPool,
    pub direction: CoinSwapDirection,
}

impl<'a> ReservedCoinPairAndIndex<'a> {
    pub fn calc_amount_out(&self, in_amount: u64) -> Result<u64> {
        self.pair.calc_amount_out(in_amount, &self.direction)
    }

    pub fn is_stable_coin_pair(&self) -> bool {
        self.pair.is_stable_coin_pair()
    }

    #[inline]
    pub fn get_input_coin(&self) -> &'a ReservedCoin {
        match self.direction {
            CoinSwapDirection::XToY => &self.pair.x,
            CoinSwapDirection::YToX => &self.pair.y,
        }
    }

    #[inline]
    pub fn get_output_coin(&self) -> &'a ReservedCoin {
        match self.direction {
            CoinSwapDirection::XToY => &self.pair.y,
            CoinSwapDirection::YToX => &self.pair.x,
        }
    }

    pub fn has_empty_reserved_amount(&self) -> bool {
        self.pair.has_empty_reserved_amount()
    }

    #[inline]
    pub fn is_match_to_coin_filter(&self, coin_pair_filters: &Option<&[RoutePairFilter]>) -> bool {
        match coin_pair_filters {
            None => true,
            Some(coin_pair_filters) => {
                if coin_pair_filters.is_empty() {
                    true
                } else {
                    for filter in *coin_pair_filters {
                        match (filter.input_coin.as_ref(), filter.output_coin.as_ref()) {
                            (Some(input_coin), Some(output_coin)) => {
                                if &self.get_input_coin().as_coin_id() == &input_coin
                                    && &self.get_output_coin().as_coin_id() == &output_coin
                                {
                                    return true;
                                }
                            },

                            (Some(input_coin), None) => {
                                if &self.get_input_coin().as_coin_id() == &input_coin {
                                    return true;
                                }
                            },

                            (None, Some(output_coin)) => {
                                if &self.get_output_coin().as_coin_id() == &output_coin {
                                    return true;
                                }
                            },

                            _ => { /* continue*/ },
                        }
                    }
                    false // didn't match any filter
                }
            },
        }
    }
}

impl<'a, 'b> Iterator for RoutePairIterator<'a, 'b>
where
    'a: 'b,
{
    type Item = ReservedCoinPairAndIndex<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self
                .pair_indices
                .next()
                .map(|index| (unsafe { self.all_pairs.get_unchecked(*index) }, index))
            {
                Some((next_pair, next_pair_index)) => {
                    if next_pair.reserverd_coin_pair.available {
                        let coin_pair = ReservedCoinPairAndIndex {
                            pair: &next_pair.reserverd_coin_pair,
                            pair_index_in_pool: CoinPairIndexInPool(*next_pair_index),
                            direction: self.direction.clone(),
                        };

                        if coin_pair.is_match_to_coin_filter(&self.coin_pair_filters) {
                            return Some(coin_pair);
                        } else {
                            // exceed to  next pair
                            continue;
                        }
                    } else {
                        // exceed to  next pair
                        continue;
                    }
                },
                None => return None,
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.pair_indices.size_hint()
    }
}
