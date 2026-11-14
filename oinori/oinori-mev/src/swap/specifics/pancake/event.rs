use crate::coins::Coin;
use anyhow::Result;
use aptos_api_types::{Event, MoveStructTag, MoveType};

use crate::resource::coin_from_generics;

use serde::{Deserialize, Serialize};

use super::POOL_ADDRESS;

#[derive(Debug, Serialize)]
pub enum PancakeEvent {
    SwapEvent(PancakeSwapEvent),
}

impl PancakeEvent {
    pub fn parse(e: &Event) -> Result<Option<Self>> {
        if e.guid.account_address != *POOL_ADDRESS {
            Ok(None)
        } else if let MoveType::Struct(struct_tag) = &e.typ {
            match (struct_tag.module.as_str(), struct_tag.name.as_str()) {
                ("swap", "SwapEvent") => Ok(Some(PancakeEvent::SwapEvent(
                    PancakeSwapEvent::from_tag_and_data(struct_tag, e.data.clone())?,
                ))),

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PancakeSwapEvent {
    pub coin_x: Coin, // in_coin
    pub coin_y: Coin, // out_coin

    pub amount_x_in: u64,
    pub amount_x_out: u64,
    pub amount_y_in: u64,
    pub amount_y_out: u64,
}

#[derive(Deserialize)]
pub struct PancakeSwapEventRaw {
    pub amount_x_in: String,
    pub amount_x_out: String,
    pub amount_y_in: String,
    pub amount_y_out: String,
}

impl PancakeSwapEvent {
    fn from_tag_and_data(type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let data: PancakeSwapEventRaw = serde_json::from_value(data)?;

        Ok(PancakeSwapEvent {
            coin_x, // in_coin
            coin_y, // out_coin

            amount_x_in: data.amount_x_in.parse::<u64>()?,
            amount_x_out: data.amount_x_out.parse::<u64>()?,
            amount_y_in: data.amount_y_in.parse::<u64>()?,
            amount_y_out: data.amount_y_out.parse::<u64>()?,
        })
    }
}
