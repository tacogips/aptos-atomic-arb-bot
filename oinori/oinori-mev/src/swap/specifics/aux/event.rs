use crate::coins::Coin;
use anyhow::Result;
use aptos_api_types::{Event, MoveStructTag, MoveType};

use super::POOL_ADDRESS;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub enum AuxEvent {
    SwapEvent(AuxSwapEvent),
}

impl AuxEvent {
    pub fn parse(e: &Event) -> Result<Option<Self>> {
        if e.guid.account_address != *POOL_ADDRESS {
            Ok(None)
        } else if let MoveType::Struct(struct_tag) = &e.typ {
            if struct_tag.address == *POOL_ADDRESS {
                match (struct_tag.module.as_str(), struct_tag.name.as_str()) {
                    ("amm", "SwapEvent") => Ok(Some(AuxEvent::SwapEvent(
                        AuxSwapEvent::from_tag_and_data(struct_tag, e.data.clone())?,
                    ))),

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

//    {
//      "guid": {
//        "creation_number": "71",
//        "account_address": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541"
//      },
//      "sequence_number": "172315",
//      "type": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541::amm::SwapEvent",
//      "data": {
//        "fee_bps": "10",
//        "in_au": "46878598",
//        "in_coin_type": "0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea::coin::T",
//        "in_reserve": "2468191996314",
//        "out_au": "998239168",
//        "out_coin_type": "0x1::aptos_coin::AptosCoin",
//        "out_reserve": "52611621074474",
//        "sender_addr": "0x89576037b3cc0b89645ea393a47787bb348272c76d6941c574b053672b848039",
//        "timestamp": "1669904073029405"
//      }
//    },

#[derive(Debug, Serialize)]
pub struct AuxSwapEvent {
    pub coin_x: Coin,          // in_coin
    pub x_amount_in: u64,      // in_reserve
    pub before_x_reserve: u64, // in_reserve

    pub coin_y: Coin,          // out_coin
    pub y_amount_out: u64,     // out_au
    pub before_y_reserve: u64, // out_reserve

    pub fee_rate: u64, // e.g if rates == 30/1000 =>  30
    pub block_timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapEventDataRaw {
    fee_bps: String,
    in_au: String,
    in_coin_type: String,
    in_reserve: String,
    out_au: String,
    out_coin_type: String,
    out_reserve: String,
    sender_addr: String,
    timestamp: String,
}

impl AuxSwapEvent {
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
    fn from_tag_and_data(_type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        //      "data": {
        //        "fee_bps": "10",
        //        "in_au": "46878598",
        //        "in_coin_type": "0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea::coin::T",
        //        "in_reserve": "2468191996314",
        //        "out_au": "998239168",
        //        "out_coin_type": "0x1::aptos_coin::AptosCoin",
        //        "out_reserve": "52611621074474",
        //        "sender_addr": "0x89576037b3cc0b89645ea393a47787bb348272c76d6941c574b053672b848039",
        //        "timestamp": "1669904073029405"
        //      }

        //SwapEventDataRaw

        let raw_data: SwapEventDataRaw = serde_json::from_value(data)?;

        let coin_x = Coin::from_str(&raw_data.in_coin_type)?;
        let coin_y = Coin::from_str(&raw_data.out_coin_type)?;

        Ok(AuxSwapEvent {
            coin_x,                                                // in_coin
            x_amount_in: raw_data.in_au.parse::<u64>()?,           // in_reserve
            before_x_reserve: raw_data.in_reserve.parse::<u64>()?, // in_reserve

            coin_y,                                                 // out_coin
            y_amount_out: raw_data.out_au.parse::<u64>()?,          // out_au
            before_y_reserve: raw_data.out_reserve.parse::<u64>()?, // out_reserve

            fee_rate: raw_data.fee_bps.parse::<u64>()?, // e.g if rates == 30/1000 =>  30
            block_timestamp: raw_data.timestamp.parse::<u64>()?,
        })
    }
}

//    {
//      "guid": {
//        "creation_number": "71",
//        "account_address": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541"
//      },
//      "sequence_number": "172315",
//      "type": "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541::amm::SwapEvent",
//      "data": {
//        "fee_bps": "10",
//        "in_au": "46878598",
//        "in_coin_type": "0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea::coin::T",
//        "in_reserve": "2468191996314",
//        "out_au": "998239168",
//        "out_coin_type": "0x1::aptos_coin::AptosCoin",
//        "out_reserve": "52611621074474",
//        "sender_addr": "0x89576037b3cc0b89645ea393a47787bb348272c76d6941c574b053672b848039",
//        "timestamp": "1669904073029405"
//      }
//    },
