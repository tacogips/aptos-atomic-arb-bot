use crate::coins::Coin;
use anyhow::Result;
use aptos_api_types::{Event, MoveStructTag, MoveType};

use crate::resource::coin_from_generics;

use serde::{Deserialize, Serialize};

use super::POOL_ADDRESS;

#[derive(Debug, Serialize)]
pub enum AnimeEvent {
    SyncEvent(AnimeSyncEvent),
    SwapEvent(AnimeSwapEvent),
}

impl AnimeEvent {
    pub fn parse(e: &Event) -> Result<Option<Self>> {
        if e.guid.account_address != *POOL_ADDRESS {
            Ok(None)
        } else if let MoveType::Struct(struct_tag) = &e.typ {
            match (struct_tag.module.as_str(), struct_tag.name.as_str()) {
                ("AnimeSwapPoolV1", "SyncEvent") => Ok(Some(AnimeEvent::SyncEvent(
                    AnimeSyncEvent::from_tag_and_data(struct_tag, e.data.clone())?,
                ))),

                ("AnimeSwapPoolV1", "SwapEvent") => Ok(Some(AnimeEvent::SwapEvent(
                    AnimeSwapEvent::from_tag_and_data(struct_tag, e.data.clone())?,
                ))),

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}

//  {
//    "guid": {
//      "creation_number": "72",
//      "account_address": "0x796900ebe1a1a54ff9e932f19c548f5c1af5c6e7d34965857ac2f7b1d1ab2cbf"
//    },
//    "sequence_number": "55756",
//    "type": "0x16fe2df00ea7dde4a63409201f7f4e536bde7bb7335526a35d05111e68aa322c::AnimeSwapPoolV1::SyncEvent<0x1::aptos_coin::AptosCoin, 0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa::asset::USDC>
//",
//    "data": {
//      "last_price_x_cumulative": "4180718562086925305274255",
//      "last_price_y_cumulative": "1197859939355067892870565794",
//      "reserve_x": "721173577978",
//      "reserve_y": "33889297414"
//    }
//  },

#[derive(Debug, Serialize)]
pub struct AnimeSyncEvent {
    pub coin_x: Coin,         // in_coin
    pub after_x_reserve: u64, // out_reserve
    pub coin_y: Coin,         // out_coin
    pub after_y_reserve: u64, // out_reserve
}

#[derive(Deserialize)]
pub struct AnimeSyncEventRaw {
    pub reserve_x: String,
    pub reserve_y: String,
}

impl AnimeSyncEvent {
    fn from_tag_and_data(type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let data: AnimeSyncEventRaw = serde_json::from_value(data)?;

        Ok(AnimeSyncEvent {
            coin_x, // in_coin
            coin_y, // out_coin
            after_x_reserve: data.reserve_x.parse::<u64>()?,
            after_y_reserve: data.reserve_y.parse::<u64>()?,
        })
    }
}

//    {
//      "guid": {
//        "creation_number": "71",
//        "account_address": "0x796900ebe1a1a54ff9e932f19c548f5c1af5c6e7d34965857ac2f7b1d1ab2cbf"
//      },
//      "sequence_number": "46666",
//      "type": "0x16fe2df00ea7dde4a63409201f7f4e536bde7bb7335526a35d05111e68aa322c::AnimeSwapPoolV1::SwapEvent<0x1::aptos_coin::AptosCoin, 0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa::asset::USDC>
//",
//      "data": {
//        "amount_x_in" : "1002748300",
//        "amount_x_out": "0",
//        "amount_y_in" : "0",
//        "amount_y_out": "47045071"
//      }
//    },

#[derive(Debug, Serialize)]
pub struct AnimeSwapEvent {
    pub coin_x: Coin, // in_coin
    pub coin_y: Coin, // out_coin

    pub amount_x_in: u64,
    pub amount_x_out: u64,
    pub amount_y_in: u64,
    pub amount_y_out: u64,
}

#[derive(Deserialize)]
pub struct AnimeSwapEventRaw {
    pub amount_x_in: String,
    pub amount_x_out: String,
    pub amount_y_in: String,
    pub amount_y_out: String,
}

impl AnimeSwapEvent {
    fn from_tag_and_data(type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let data: AnimeSwapEventRaw = serde_json::from_value(data)?;

        Ok(AnimeSwapEvent {
            coin_x, // in_coin
            coin_y, // out_coin

            amount_x_in: data.amount_x_in.parse::<u64>()?,
            amount_x_out: data.amount_x_out.parse::<u64>()?,
            amount_y_in: data.amount_y_in.parse::<u64>()?,
            amount_y_out: data.amount_y_out.parse::<u64>()?,
        })
    }
}
