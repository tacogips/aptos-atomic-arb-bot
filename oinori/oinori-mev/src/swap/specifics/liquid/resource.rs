use crate::coins::Coin;
use anyhow::{anyhow, Result};

use crate::resource::{
    coin_from_generics, get_value_from_struct_value, get_value_from_struct_with_type,
    struct_from_generics,
};

use aptos_api_types::{MoveStructTag, MoveStructValue, WriteSetChange};

use super::CONTRACT_ADDRESS;
use super::POOL_ADDRESS;

use crate::swap::ConvertResource;
use crate::swap::{AmmCurveType, ReservedCoin, ReservedCoinPair};
use serde::{Deserialize, Serialize};

////TODO remove
//const LIQUID_POOL_MODULE_AND_NAME: (&str, &str) = ("liquidity_pool", "LiquidityPool");
//const UNCORRELATED_CURVE_MODULE_AND_NAME: (&str, &str) = ("curves", "Uncorrelated");

const FEE_SCALE: u64 = 10_000;

#[derive(Debug, Serialize, Clone)]
pub enum LiquidResource {
    LiquidityPool(LiquidPoolResource),
}

impl ConvertResource for LiquidResource {
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
                ("liquidity_pool", "LiquidityPool") => {
                    let liquid_resouce = LiquidPoolResource::parse_tag_and_data(type_tag, data)?;
                    //TODO not use stable
                    //// ignore stable curve
                    //if liquid_resouce.curve_type == LiquidCurveType::Stable {
                    //    Ok(None)
                    //} else {
                    //    Ok(Some(LiquidResource::LiquidityPool(liquid_resouce)))
                    //}
                    Ok(Some(LiquidResource::LiquidityPool(liquid_resouce)))
                },

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let Self::LiquidityPool(LiquidPoolResource {
            coin_x,
            coin_y,
            curve_type,
            fee_rate,
            coin_x_reserve,
            coin_y_reserve,
            stable_scale_x,
            stable_scale_y,
            last_block_timestamp: _,
            locked,
        }) = self;

        Ok(Some(ReservedCoinPair {
            x: ReservedCoin::new(coin_x, coin_x_reserve),
            y: ReservedCoin::new(coin_y, coin_y_reserve),
            fee_rate,
            amount_scale: FEE_SCALE,
            amm_curve_type: curve_type.as_amm_curve_type(),
            stable_scale_x,
            stable_scale_y,
            available: !locked,
        }))
    }
}

#[derive(Debug, Serialize, PartialEq, Clone)]
pub enum LiquidCurveType {
    Stable,
    Uncorrelated,
}

impl LiquidCurveType {
    pub fn as_amm_curve_type(&self) -> AmmCurveType {
        match &self {
            Self::Stable => AmmCurveType::LiquidStable,
            Self::Uncorrelated => AmmCurveType::UniswapV2,
        }
    }

    //"type": 0x190d44266241744264b964a37b8f09863167a12d3e70cda39376cfb4e3561e12::curves::Uncorrelated",
    pub fn from_tag(tag: &MoveStructTag) -> Result<Self> {
        if tag.module.as_str() == "curves" {
            match tag.name.as_str() {
                "Uncorrelated" => Ok(Self::Uncorrelated),
                "Stable" => Ok(Self::Stable),
                _ => Err(anyhow!("invalid curve {:?}", tag)),
            }
        } else {
            Err(anyhow!("invalid curve {:?}", tag))
        }
    }
}

impl LiquidResource {
    pub fn resource_type(&self) -> LiquidResourceType {
        match &self {
            LiquidResource::LiquidityPool(_) => LiquidResourceType::LiquidityPool,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct LiquidPoolResource {
    pub coin_x: Coin,
    pub coin_y: Coin,
    pub curve_type: LiquidCurveType,
    pub fee_rate: u64,
    pub coin_x_reserve: u64,
    pub coin_y_reserve: u64,
    pub last_block_timestamp: u64,
    pub stable_scale_x: u64,
    pub stable_scale_y: u64,
    pub locked: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinValue {
    value: String,
}

impl LiquidPoolResource {
    fn parse_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;
        let curve_type = struct_from_generics(&type_tag.generic_type_params, 2)?;
        let curve_type = LiquidCurveType::from_tag(curve_type)?;

        let coin_x_reserve = get_value_from_struct_with_type!(data, "coin_x_reserve", CoinValue)?;
        let coin_y_reserve = get_value_from_struct_with_type!(data, "coin_y_reserve", CoinValue)?;
        let fee_rate = get_value_from_struct_with_type!(data, "fee", String)?;

        let x_scale = get_value_from_struct_with_type!(data, "x_scale", String)?;
        let y_scale = get_value_from_struct_with_type!(data, "x_scale", String)?;

        let last_block_timestamp =
            get_value_from_struct_with_type!(data, "last_block_timestamp", String)?;

        let locked = get_value_from_struct_with_type!(data, "locked", bool)?;
        Ok(LiquidPoolResource {
            coin_x,
            coin_y,
            curve_type,
            fee_rate: fee_rate.parse::<u64>()?,
            coin_x_reserve: coin_x_reserve.value.parse::<u64>()?,
            coin_y_reserve: coin_y_reserve.value.parse::<u64>()?,
            last_block_timestamp: last_block_timestamp.parse::<u64>()?,
            stable_scale_x: x_scale.parse::<u64>()?,
            stable_scale_y: y_scale.parse::<u64>()?,
            locked,
        })
    }
}

pub enum LiquidResourceType {
    LiquidityPool,
}
