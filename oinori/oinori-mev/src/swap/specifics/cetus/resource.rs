use crate::coins::Coin;
use crate::resource::{
    coin_from_generics, get_value_from_struct_value, get_value_from_struct_with_type,
};

use anyhow::Result;
use aptos_api_types::{MoveStructTag, MoveStructValue, WriteSetChange};

use super::POOL_ADDRESS;
use crate::swap::ConvertResource;
use crate::swap::{AmmCurveType, ReservedCoin, ReservedCoinPair};
use serde::{Deserialize, Serialize};

const AMOUNT_SCALE: u64 = 1_000;

// 0.2% fee rate
const FEE_RATE: u64 = 2;

#[derive(Debug, Serialize, Clone)]
pub enum CetusResource {
    Pool(CetusPool),
}

impl ConvertResource for CetusResource {
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
                ("amm_swap", "Pool") => Ok(Some(CetusResource::Pool(
                    CetusPool::from_tag_and_data(type_tag, data)?,
                ))),

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let Self::Pool(CetusPool {
            coin_x,
            coin_y,
            coin_x_reserve,
            coin_y_reserve,

            locked_liquidity: _,
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
pub struct CetusPool {
    pub coin_x: Coin,
    pub coin_y: Coin,

    pub coin_x_reserve: u64,
    pub coin_y_reserve: u64,

    pub locked_liquidity: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinValue {
    value: String,
}

impl CetusPool {
    fn from_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let coin_x_reserve = get_value_from_struct_with_type!(data, "coin_a", CoinValue)?;
        let coin_y_reserve = get_value_from_struct_with_type!(data, "coin_b", CoinValue)?;
        let locked_liquidity =
            get_value_from_struct_with_type!(data, "locked_liquidity", CoinValue)?;

        Ok(CetusPool {
            coin_x,
            coin_y,
            coin_x_reserve: coin_x_reserve.value.parse::<u64>()?,
            coin_y_reserve: coin_y_reserve.value.parse::<u64>()?,
            locked_liquidity: locked_liquidity.value.parse::<u64>()?,
        })
    }
}
