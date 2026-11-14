use crate::coins::Coin;
use crate::resource::{
    coin_from_generics, get_value_from_struct_value, get_value_from_struct_with_type,
};

use crate::swap::amm::U256;
use crate::swap::ConvertResource;
use crate::swap::{AmmCurveType, ReservedCoin, ReservedCoinPair};
use anyhow::Result;
use aptos_api_types::{MoveStructTag, MoveStructValue, WriteSetChange};

use serde::{Deserialize, Serialize};

use super::POOL_ADDRESS;

const AMOUNT_SCALE: u64 = 10_000;
// 0.25% fee rate
const FEE_RATE: u64 = 25;

#[derive(Debug, Serialize, Clone)]
pub enum PancakeResource {
    PairMetaData(PancakePairMetadata),
}

impl ConvertResource for PancakeResource {
    fn from_change(e: &WriteSetChange) -> Result<Option<Self>> {
        if let WriteSetChange::WriteResource(write_resource) = e {
            if write_resource.address != *POOL_ADDRESS {
                Ok(None)
            } else {
                let type_tag = &write_resource.data.typ;
                let move_struct_value = &write_resource.data.data;
                Self::from_tag_and_data(type_tag, move_struct_value)
            }
        } else {
            Ok(None)
        }
    }

    fn from_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Option<Self>> {
        if type_tag.address == *POOL_ADDRESS {
            match (type_tag.module.as_str(), type_tag.name.as_str()) {
                ("swap", "TokenPairMetadata") => Ok(Some(PancakeResource::PairMetaData(
                    PancakePairMetadata::from_tag_and_data(type_tag, data)?,
                ))),

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let Self::PairMetaData(PancakePairMetadata {
            coin_x,
            coin_y,
            coin_x_reserve,
            coin_y_reserve,
            k_last: _,
        }) = self;

        Ok(Some(ReservedCoinPair {
            x: ReservedCoin::new(coin_x, coin_x_reserve),
            y: ReservedCoin::new(coin_y, coin_y_reserve),
            fee_rate: FEE_RATE,
            amount_scale: AMOUNT_SCALE,
            amm_curve_type: AmmCurveType::UniswapV2,
            stable_scale_x: 0,
            stable_scale_y: 0,
            available: true,
        }))
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct PancakePairMetadata {
    pub coin_x: Coin,
    pub coin_y: Coin,

    pub k_last: u128, // (reserves.reserve_x as u128) * (reserves.reserve_y as u128)
    pub coin_x_reserve: u64,
    pub coin_y_reserve: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinValue {
    value: String,
}

impl PancakePairMetadata {
    fn from_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let coin_x_reserve = get_value_from_struct_with_type!(data, "balance_x", CoinValue)?;
        let coin_y_reserve = get_value_from_struct_with_type!(data, "balance_y", CoinValue)?;

        let k_last = get_value_from_struct_with_type!(data, "k_last", String)?;

        Ok(PancakePairMetadata {
            coin_x,
            coin_y,
            k_last: (k_last.parse::<U256>()? / 100000000).as_u128(),
            coin_x_reserve: coin_x_reserve.value.parse::<u64>()?,
            coin_y_reserve: coin_y_reserve.value.parse::<u64>()?,
        })
    }
}
