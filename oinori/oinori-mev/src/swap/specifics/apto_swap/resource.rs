use super::POOL_ADDRESS;
use crate::coins::Coin;
use crate::resource::{
    coin_from_generics, get_value_from_struct_value, get_value_from_struct_with_type,
};
use crate::swap::ConvertResource;
use crate::swap::{AdditionalFee, AmmCurveType, ReservedCoin, ReservedCoinPair};
use anyhow::{anyhow, Result};
use aptos_api_types::{MoveStructTag, MoveStructValue, WriteSetChange};
use serde::{Deserialize, Serialize};

const AMOUNT_SCALE: u64 = 10000;
#[derive(Debug, Serialize, Clone)]
pub enum AptoSwapResource {
    Pool(AptoSwapPool),
}

impl ConvertResource for AptoSwapResource {
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
                ("pool", "Pool") => Ok(Some(AptoSwapResource::Pool(
                    AptoSwapPool::from_tag_and_data(type_tag, data)?,
                ))),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let Self::Pool(AptoSwapPool {
            coin_x,
            coin_y,
            reserve_x,
            reserve_y,

            pool_type,
            stable_x_scale,
            stable_y_scale,
            lp_fee,
            incentive_fee,

            admin_fee: _,
            connect_fee: _,
            admin_fee_with_direction,

            last_trade_time: _,
            ..
        }) = self;

        Ok(Some(ReservedCoinPair {
            x: ReservedCoin::new(coin_x, reserve_x),
            y: ReservedCoin::new(coin_y, reserve_y),
            fee_rate: lp_fee + incentive_fee,
            amount_scale: AMOUNT_SCALE,
            amm_curve_type: pool_type.to_amm_curve_type(&admin_fee_with_direction),
            stable_scale_x: stable_x_scale,
            stable_scale_y: stable_y_scale,
            // stable pool not yet implemented
            available: pool_type == PoolType::UniswapV2,
        }))
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub enum PoolType {
    Stable,
    UniswapV2,
}

impl PoolType {
    pub fn from_u8(s: u8) -> Result<Self> {
        match s {
            100 => Ok(Self::UniswapV2),
            101 => Ok(Self::Stable),
            _ => Err(anyhow!("unknown pool type {s}")),
        }
    }

    pub fn to_amm_curve_type(&self, additional_fee: &AdditionalFee) -> AmmCurveType {
        match self {
            Self::Stable => AmmCurveType::CurveStableWithAdditionalFee(additional_fee.clone()),
            Self::UniswapV2 => AmmCurveType::UniswapV2WithAdditionalFee(additional_fee.clone()),
        }
    }
}

pub fn additional_fee_with_direction(fee_direction: u8, fee_rate: u64) -> Result<AdditionalFee> {
    match fee_direction {
        200 => Ok(AdditionalFee::FromX(fee_rate)),
        201 => Ok(AdditionalFee::FromY(fee_rate)),
        _ => Err(anyhow!("unknown AdminFeeSourceSide {fee_direction}")),
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct AptoSwapPool {
    pub coin_x: Coin,
    pub coin_y: Coin,
    pub reserve_x: u64,
    pub reserve_y: u64,

    pub pool_type: PoolType,
    pub stable_x_scale: u64,
    pub stable_y_scale: u64,
    pub lp_fee: u64,
    pub incentive_fee: u64,

    pub admin_fee: u64,
    pub connect_fee: u64,
    pub admin_fee_with_direction: AdditionalFee,

    pub last_trade_time: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinValue {
    value: String,
}

impl AptoSwapPool {
    //      "data": {
    //       "type": "0xa5d3ac4d429052674ed38adc62d010e52d7c24ca159194d17ddc196ddb7e480b::pool::Pool<0x1::aptos_coin::AptosCoin, 0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa::asset::USDC>",
    //       "data": {
    //         "admin_fee": "0",
    //         "connect_fee": "0",
    //         "fee_direction": 201,
    //         "freeze": false,
    //         "incentive_fee": "3",
    //         "index": "2",
    //         "last_trade_time": "1669904041",
    //         "lp_fee": "27",
    //         "lsp_supply": "33482250492",
    //         "pool_type": 100,
    //         "snapshot_last_capture_time": "1669904041",
    //         "stable_amp": "0",
    //         "stable_x_scale": "0",
    //         "stable_y_scale": "0",
    //         "withdraw_fee": "10",
    //         "x": {
    //           "value": "170631025235"
    //         },
    //         "y": {
    //           "value": "8011719697"
    //         }
    //       }
    //     }
    //   },
    //

    fn from_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let reserve_x = get_value_from_struct_with_type!(data, "x", CoinValue)?;
        let reserve_y = get_value_from_struct_with_type!(data, "y", CoinValue)?;

        let pool_type =
            PoolType::from_u8(get_value_from_struct_with_type!(data, "pool_type", u8)?)?;

        let stable_x_scale =
            get_value_from_struct_with_type!(data, "stable_x_scale", String)?.parse::<u64>()?;
        let stable_y_scale =
            get_value_from_struct_with_type!(data, "stable_y_scale", String)?.parse::<u64>()?;

        let lp_fee = get_value_from_struct_with_type!(data, "lp_fee", String)?.parse::<u64>()?;
        let incentive_fee =
            get_value_from_struct_with_type!(data, "incentive_fee", String)?.parse::<u64>()?;

        let admin_fee =
            get_value_from_struct_with_type!(data, "admin_fee", String)?.parse::<u64>()?;
        let connect_fee =
            get_value_from_struct_with_type!(data, "connect_fee", String)?.parse::<u64>()?;

        let additional_fee = admin_fee + connect_fee;
        let admin_fee_with_direction = additional_fee_with_direction(
            get_value_from_struct_with_type!(data, "fee_direction", u8)?,
            additional_fee,
        )?;

        let last_trade_time =
            get_value_from_struct_with_type!(data, "last_trade_time", String)?.parse::<u64>()?;

        Ok(AptoSwapPool {
            coin_x,
            coin_y,

            reserve_x: reserve_x.value.parse::<u64>()?,
            reserve_y: reserve_y.value.parse::<u64>()?,
            pool_type,

            stable_x_scale,
            stable_y_scale,

            lp_fee,
            incentive_fee,

            admin_fee_with_direction,
            admin_fee,
            connect_fee,
            last_trade_time,
        })
    }
}
