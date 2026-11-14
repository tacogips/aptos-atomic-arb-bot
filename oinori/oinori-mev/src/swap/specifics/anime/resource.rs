use crate::coins::Coin;
use crate::resource::{
    coin_from_generics, get_value_from_struct_value, get_value_from_struct_with_type,
};
use crate::swap::ConvertResource;
use crate::swap::{AmmCurveType, ReservedCoin, ReservedCoinPair};
use anyhow::Result;
use aptos_api_types::{MoveStructTag, MoveStructValue, WriteSetChange};
use serde::{Deserialize, Serialize};

use super::{CONTRACT_ADDRESS, POOL_ADDRESS};

const AMOUNT_SCALE: u64 = 10_000;
// 0.3% fee rate
const FEE_RATE: u64 = 30;

#[derive(Debug, Serialize, Clone)]
pub enum AnimeResource {
    Pool(AnimePool),
}

impl ConvertResource for AnimeResource {
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
        if type_tag.address == *CONTRACT_ADDRESS {
            match (type_tag.module.as_str(), type_tag.name.as_str()) {
                ("AnimeSwapPoolV1", "LiquidityPool") => Ok(Some(AnimeResource::Pool(
                    AnimePool::from_tag_and_data(type_tag, data)?,
                ))),

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let Self::Pool(AnimePool {
            coin_x,
            coin_y,

            k_last: _,
            locked,
            coin_x_reserve,
            coin_y_reserve,
            last_block_timestamp: _,
        }) = self;

        Ok(Some(ReservedCoinPair {
            x: ReservedCoin::new(coin_x, coin_x_reserve),
            y: ReservedCoin::new(coin_y, coin_y_reserve),
            fee_rate: FEE_RATE,
            amount_scale: AMOUNT_SCALE,
            amm_curve_type: AmmCurveType::UniswapV2,
            stable_scale_x: 0,
            stable_scale_y: 0,
            available: !locked,
        }))
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct AnimePool {
    pub coin_x: Coin,
    pub coin_y: Coin,

    pub k_last: u128,
    pub locked: bool,
    pub coin_x_reserve: u64,
    pub coin_y_reserve: u64,
    pub last_block_timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinValue {
    value: String,
}

impl AnimePool {
    //    {
    //      "type": "write_resource",
    //      "address": "0x796900ebe1a1a54ff9e932f19c548f5c1af5c6e7d34965857ac2f7b1d1ab2cbf",
    //      "state_key_hash": "0x159e60c28c2b296f7f8b85361dacdcc66fa3f55af4395369565e119d9cb83559",
    //      "data": {
    //        "type": "0x16fe2df00ea7dde4a63409201f7f4e536bde7bb7335526a35d05111e68aa322c::AnimeSwapPoolV1::LiquidityPool<0x1::aptos_coin::AptosCoin, 0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa::asset:
    //:USDC>",
    //        "data": {
    //          "coin_x_reserve": {
    //            "value": "721173577978"
    //          },
    //          "coin_y_reserve": {
    //            "value": "33889297414"
    //          },
    //          "k_last": "24438679234929637212957",
    //          "last_block_timestamp": "1669904040",
    //          "last_price_x_cumulative": "4180718562086925305274255",
    //          "last_price_y_cumulative": "1197859939355067892870565794",
    //          "locked": false,
    //          "lp_burn_cap": {
    //            "dummy_field": false
    //          },
    //          "lp_freeze_cap": {
    //            "dummy_field": false
    //          },
    //          "lp_mint_cap": {
    //            "dummy_field": false
    //          }
    //        }
    //      }
    //    },
    //

    fn from_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let coin_x_reserve = get_value_from_struct_with_type!(data, "coin_x_reserve", CoinValue)?;
        let coin_y_reserve = get_value_from_struct_with_type!(data, "coin_y_reserve", CoinValue)?;

        let k_last = get_value_from_struct_with_type!(data, "k_last", String)?;

        let last_block_timestamp =
            get_value_from_struct_with_type!(data, "last_block_timestamp", String)?;
        let locked = get_value_from_struct_with_type!(data, "locked", bool)?;

        Ok(AnimePool {
            coin_x,
            coin_y,
            k_last: k_last.parse::<u128>()?,
            locked,
            coin_x_reserve: coin_x_reserve.value.parse::<u64>()?,
            coin_y_reserve: coin_y_reserve.value.parse::<u64>()?,
            last_block_timestamp: last_block_timestamp.parse::<u64>()?,
        })
    }
}
