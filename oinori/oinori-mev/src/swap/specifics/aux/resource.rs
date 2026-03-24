use super::POOL_ADDRESS;
use crate::coins::Coin;
use crate::resource::{
    coin_from_generics, get_value_from_struct_value, get_value_from_struct_with_type,
};
use crate::swap::ConvertResource;
use crate::swap::{AmmCurveType, ReservedCoin, ReservedCoinPair};
use anyhow::Result;
use aptos_api_types::{MoveStructTag, MoveStructValue, WriteSetChange};
use serde::{Deserialize, Serialize};

const AMOUNT_SCALE: u64 = 10_000;
#[derive(Debug, Serialize, Clone)]
pub enum AuxResource {
    Pool(AuxPool),
}

impl ConvertResource for AuxResource {
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
                ("amm", "Pool") => Ok(Some(AuxResource::Pool(AuxPool::from_tag_and_data(
                    type_tag, data,
                )?))),

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let Self::Pool(AuxPool {
            coin_x,
            coin_y,
            coin_x_reserve,
            coin_y_reserve,
            fee_rate,
            frozen,
            ..
        }) = self;

        Ok(Some(ReservedCoinPair {
            x: ReservedCoin::new(coin_x, coin_x_reserve),
            y: ReservedCoin::new(coin_y, coin_y_reserve),
            fee_rate,
            amount_scale: AMOUNT_SCALE,
            amm_curve_type: AmmCurveType::UniswapV2,
            stable_scale_x: 0,
            stable_scale_y: 0,
            available: !frozen,
        }))
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct AuxPool {
    pub coin_x: Coin,
    pub coin_y: Coin,

    pub fee_rate: u64, // fee_bps
    pub frozen: bool,
    pub coin_x_reserve: u64,
    pub coin_y_reserve: u64,
    pub last_block_timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinValue {
    value: String,
}

impl AuxPool {
    //    {
    //      "type": "write_resource",
    //      "address": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541",
    //      "state_key_hash": "0xf8a88de8c3d8a5fb57e2c37e400609d141e36deef324545f35c693f44f55f040",
    //      "data": {
    //        "type": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541::amm::Pool<0x1::aptos_coin::AptosCoin, 0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea::coin::T>",
    //        "data": {
    //          "add_liquidity_events": {
    //            "counter": "2571",
    //            "guid": {
    //              "id": {
    //                "addr": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541",
    //                "creation_num": "72"
    //              }
    //            }
    //          },
    //          "fee_bps": "10",
    //          "frozen": false,
    //          "lp_burn": {
    //            "dummy_field": false
    //          },
    //          "lp_mint": {
    //            "dummy_field": false
    //          },
    //          "remove_liquidity_events": {
    //            "counter": "1017",
    //            "guid": {
    //              "id": {
    //                "addr": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541",
    //                "creation_num": "73"
    //              }
    //            }
    //          },
    //          "swap_events": {
    //            "counter": "172316",
    //            "guid": {
    //              "id": {
    //                "addr": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541",
    //                "creation_num": "71"
    //              }
    //            }
    //          },
    //          "timestamp": "1669904073029405",
    //          "x_reserve": {
    //            "value": "52610622835306"
    //          },
    //          "y_reserve": {
    //            "value": "2468238874912"
    //          }
    //        }
    //      }
    fn from_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let coin_x_reserve = get_value_from_struct_with_type!(data, "x_reserve", CoinValue)?;
        let coin_y_reserve = get_value_from_struct_with_type!(data, "y_reserve", CoinValue)?;
        let last_block_timestamp = get_value_from_struct_with_type!(data, "timestamp", String)?;
        let fee_rate = get_value_from_struct_with_type!(data, "fee_bps", String)?;
        let frozen = get_value_from_struct_with_type!(data, "frozen", bool)?;

        Ok(AuxPool {
            coin_x,
            coin_y,

            fee_rate: fee_rate.parse::<u64>()?,
            frozen,
            coin_x_reserve: coin_x_reserve.value.parse::<u64>()?,
            coin_y_reserve: coin_y_reserve.value.parse::<u64>()?,
            last_block_timestamp: last_block_timestamp.parse::<u64>()?,
        })
    }
}
