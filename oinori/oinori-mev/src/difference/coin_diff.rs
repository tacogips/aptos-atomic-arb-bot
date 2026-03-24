use crate::coins::{Coin, CoinAmount, CoinId};
use crate::hasher::*;
use anyhow::Result;
use aptos_api_types::MoveConverter;
use aptos_storage_interface::state_view::DbStateView;
use num_integer::Integer;
use serde::Serialize;
use std::collections::BTreeMap;

use aptos_storage_interface::DbReader;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct CoinAmountSub {
    pub coin: Coin,
    pub amount: i64,
}

impl CoinAmountSub {
    pub fn decimal_amount_str(&self) -> String {
        let (q, r) = self.amount.div_rem(&10i64.pow(*self.coin.decimal as u32));
        format!("{}.{}", q, r)
    }

    pub fn new(before: &CoinAmount, after: &CoinAmount) -> CoinAmountSub {
        Self {
            coin: after.coin.clone(),
            amount: after.amount as i64 - before.amount as i64,
        }
    }

    pub fn single(after: &CoinAmount) -> CoinAmountSub {
        Self {
            coin: after.coin.clone(),
            amount: after.amount as i64,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.amount == 0
    }
}

#[derive(Debug, Serialize, Default)]
pub struct CoinAmountDiffs {
    pub coin_diffs: Vec<CoinAmountDiff>,
}
impl CoinAmountDiffs {
    pub fn group_by_after_versoin(self) -> BTreeMap<u64, Vec<CoinAmountDiff>> {
        self.coin_diffs
            .into_iter()
            .filter(|d| !d.is_zero())
            .into_iter()
            .fold(
                BTreeMap::<u64, Vec<CoinAmountDiff>>::new(),
                |mut acc, each| {
                    let diffs = acc.entry(each.after_version).or_insert(vec![]);
                    diffs.push(each);
                    acc
                },
            )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CoinAmountDiff {
    pub before_version: u64,
    pub after_version: u64,
    pub before_amount: Option<CoinAmount>,
    pub after_amount: CoinAmount,
    pub diff_amount: CoinAmountSub,
}

impl CoinAmountDiff {
    pub fn is_apt_diff(&self) -> bool {
        self.after_amount.coin.is_apt_coin()
    }
    pub fn find_apts_coin(diffs: &[CoinAmountDiff]) -> Option<&CoinAmountDiff> {
        diffs
            .iter()
            .find(|diff| diff.after_amount.coin.is_apt_coin())
    }

    pub fn is_zero(&self) -> bool {
        self.diff_amount.amount == 0
    }

    pub fn coin_id(&self) -> &CoinId {
        self.after_amount.coin.as_coin_id()
    }

    pub fn from(
        before_version: u64,
        before_amount: Option<&CoinAmount>,
        after_version: u64,
        after_amount: &CoinAmount,
    ) -> Self {
        let diff_amount = if let Some(before_amount) = before_amount {
            CoinAmountSub::new(before_amount, after_amount)
        } else {
            CoinAmountSub::single(after_amount)
        };

        Self {
            before_version,
            before_amount: before_amount.cloned(),
            after_version,
            after_amount: after_amount.clone(),
            diff_amount,
        }
    }

    pub fn diffs(
        before_version: u64,
        before_coins: &Vec<CoinAmount>,
        after_version: u64,
        after_coins: &Vec<CoinAmount>,
    ) -> Vec<CoinAmountDiff> {
        let befores = before_coins
            .iter()
            .map(|coin_amount| (coin_amount.coin.as_coin_id(), coin_amount))
            .collect::<HashMap<&CoinId, &CoinAmount>>();

        after_coins
            .iter()
            .map(|each_after_coin| {
                CoinAmountDiff::from(
                    before_version,
                    befores.get(each_after_coin.coin.as_coin_id()).copied(),
                    after_version,
                    each_after_coin,
                )
            })
            .collect()
    }

    pub fn from_versions(
        address_hex: &str,
        target_versions: Vec<u64>,
        db_reader: Arc<dyn DbReader>,
        move_converter: &MoveConverter<DbStateView>,
    ) -> Result<CoinAmountDiffs> {
        match target_versions.first() {
            None => Ok(CoinAmountDiffs::default()),
            Some(first_version) => {
                let mut before_version: u64 = *first_version;
                let mut before_coin_amounts = CoinAmount::load_resource(
                    address_hex,
                    false,
                    db_reader.clone(),
                    Some(*first_version),
                    move_converter,
                )?;

                let mut after_coin_amounts: Vec<CoinAmount>;
                let mut coin_diffs = Vec::<CoinAmountDiff>::new();

                for each_after_version in target_versions[1..].iter() {
                    after_coin_amounts = CoinAmount::load_resource(
                        address_hex,
                        false,
                        db_reader.clone(),
                        Some(*each_after_version),
                        move_converter,
                    )?;
                    coin_diffs.append(&mut CoinAmountDiff::diffs(
                        before_version,
                        &before_coin_amounts,
                        *each_after_version,
                        &after_coin_amounts,
                    ));

                    before_version = *each_after_version;
                    before_coin_amounts = after_coin_amounts;
                }

                Ok(CoinAmountDiffs { coin_diffs })
            },
        }
    }
}
