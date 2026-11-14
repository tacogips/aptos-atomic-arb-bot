use crate::coins::Coin;
use crate::resource::{coin_from_generics, struct_from_generics};
use crate::swap::liquid::resource::LiquidCurveType;
use anyhow::Result;
use aptos_api_types::{Event, MoveStructTag, MoveType};
use serde::{Deserialize, Serialize};

use super::CONTRACT_ADDRESS;
use super::POOL_ADDRESS;

#[derive(Debug, Serialize)]
pub enum LiquidEvent {
    SwapEvent(LiquidSwapEvent),
    DaoDepositEvent(DaoDepositEvent),
}

impl LiquidEvent {
    pub fn parse(e: &Event) -> Result<Option<Self>> {
        if e.guid.account_address != *POOL_ADDRESS {
            Ok(None)
        } else if let MoveType::Struct(struct_tag) = &e.typ {
            if struct_tag.address == *CONTRACT_ADDRESS {
                match (struct_tag.module.as_str(), struct_tag.name.as_str()) {
                    ("liquidity_pool", "SwapEvent") => Ok(Some(LiquidEvent::SwapEvent(
                        LiquidSwapEvent::from_tag_and_data(struct_tag, e.data.clone())?,
                    ))),

                    ("dao_storage", "CoinDepositedEvent") => {
                        Ok(Some(LiquidEvent::DaoDepositEvent(
                            DaoDepositEvent::from_tag_and_data(struct_tag, e.data.clone())?,
                        )))
                    }
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LiquidSwapEvent {
    pub coin_x: Coin,
    pub coin_y: Coin,
    pub curve_type: LiquidCurveType,
    pub x_in: u64,
    pub x_out: u64,
    pub y_in: u64,
    pub y_out: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapEventDataRaw {
    pub x_in: String,
    pub x_out: String,
    pub y_in: String,
    pub y_out: String,
}

impl LiquidSwapEvent {
    //"type": "0x190d44266241744264b964a37b8f09863167a12d3e70cda39376cfb4e3561e12::liquidity_pool::SwapEvent<
    //0xa2eda21a58856fda86451436513b867c97eecb4ba099da5775520e0f7492e852::coin::T
    //, 0x1::aptos_coin::AptosCoin,
    //0x190d44266241744264b964a37b8f09863167a12d3e70cda39376cfb4e3561e12::curves::Uncorrelated>",
    //  "data": {
    //    "x_in": "6962106",
    //    "x_out": "0",
    //    "y_in": "0",
    //    "y_out": "177492795"
    //  }
    fn from_tag_and_data(type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;
        let curve_type = struct_from_generics(&type_tag.generic_type_params, 2)?;
        let curve_type = LiquidCurveType::from_tag(curve_type)?;

        let SwapEventDataRaw {
            x_in,
            x_out,
            y_in,
            y_out,
        } = serde_json::from_value(data)?;

        Ok(LiquidSwapEvent {
            coin_x,
            coin_y,
            curve_type,
            x_in: x_in.parse::<u64>()?,
            x_out: x_out.parse::<u64>()?,
            y_in: y_in.parse::<u64>()?,
            y_out: y_out.parse::<u64>()?,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct DaoDepositEvent {
    pub x_val: u64,
    pub y_val: u64,
}

#[derive(Debug, Deserialize)]
pub struct DaoDepositDataRaw {
    pub x_val: String,
    pub y_val: String,
}

impl DaoDepositEvent {
    //      "sequence_number": "13357",
    //      "type": "0x190d44266241744264b964a37b8f09863167a12d3e70cda39376cfb4e3561e12::dao_storage::CoinDepositedEvent<0xa2eda21a58856fda86451436513b867c97eecb4ba099da5775520e0f7492e852::coin::T, 0x1::aptos_coin::AptosCoin, 0x190d44266241744264b964a37b8f09863167a12d3e70cda39376cfb4e3561e12::curves::Uncorrelated>",
    //      "data": {
    //        "x_val": "6962",
    //        "y_val": "0"
    //      }

    fn from_tag_and_data(_type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        let DaoDepositDataRaw { x_val, y_val } = serde_json::from_value(data)?;

        Ok(DaoDepositEvent {
            x_val: x_val.parse::<u64>()?,
            y_val: y_val.parse::<u64>()?,
        })
    }
}
