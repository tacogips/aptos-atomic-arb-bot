use crate::coins::Coin;
use anyhow::Result;
use aptos_api_types::{Event, MoveStructTag, MoveType};

use serde::{Deserialize, Serialize};

use super::POOL_ADDRESS;

#[derive(Debug, Serialize)]
pub enum CetusEvent {
    SwapEvent(CetusSwapEvent),
}

impl CetusEvent {
    pub fn parse(e: &Event) -> Result<Option<Self>> {
        if e.guid.account_address != *POOL_ADDRESS {
            Ok(None)
        } else if let MoveType::Struct(struct_tag) = &e.typ {
            match (struct_tag.module.as_str(), struct_tag.name.as_str()) {
                ("amm_swap", "SwapEvent") => Ok(Some(CetusEvent::SwapEvent(
                    CetusSwapEvent::from_tag_and_data(struct_tag, e.data.clone())?,
                ))),

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CetusSwapEvent {
    pub coin_x: Coin, // in_coin
    pub coin_y: Coin, // out_coin

    pub amount_x_in: u64,
    pub amount_x_out: u64,
    pub amount_y_in: u64,
    pub amount_y_out: u64,
}

#[derive(Deserialize)]
pub struct CetusSwapEventRaw {
    pub a_in: String,
    pub a_out: String,
    pub b_in: String,
    pub b_out: String,
    pub coin_a_info: CoinInfoRaw,
    pub coin_b_info: CoinInfoRaw,
}

#[derive(Deserialize)]
pub struct CoinInfoRaw {
    pub account_address: String,
    pub module_name: String,
    pub struct_name: String,
}

impl CetusSwapEvent {
    fn from_tag_and_data(_type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        let data: CetusSwapEventRaw = serde_json::from_value(data)?;

        let coin_x = Coin::from_hex_strs(
            &data.coin_a_info.account_address,
            &data.coin_a_info.module_name,
            &data.coin_a_info.struct_name,
        )?;

        let coin_y = Coin::from_hex_strs(
            &data.coin_b_info.account_address,
            &data.coin_b_info.module_name,
            &data.coin_b_info.struct_name,
        )?;

        Ok(CetusSwapEvent {
            coin_x,
            coin_y,

            amount_x_in: data.a_in.parse::<u64>()?,
            amount_x_out: data.a_out.parse::<u64>()?,
            amount_y_in: data.b_in.parse::<u64>()?,
            amount_y_out: data.b_out.parse::<u64>()?,
        })
    }
}
