use crate::coins::Coin;

use anyhow::Result;
use aptos_api_types::{Event, MoveStructTag, MoveType};

use serde::{Deserialize, Serialize};

use super::POOL_ADDRESS;

#[derive(Debug, Serialize)]
pub enum ObricEvent {
    SwapEvent(ObricSwapEvent),
}

impl ObricEvent {
    pub fn parse(e: &Event) -> Result<Option<Self>> {
        if e.guid.account_address != *POOL_ADDRESS {
            Ok(None)
        } else if let MoveType::Struct(struct_tag) = &e.typ {
            if struct_tag.address == *POOL_ADDRESS {
                match (struct_tag.module.as_str(), struct_tag.name.as_str()) {
                    ("piece_swap", "SwapEvent") => Ok(Some(ObricEvent::SwapEvent(
                        ObricSwapEvent::from_tag_and_data(struct_tag, e.data.clone())?,
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
pub struct ObricSwapEvent {
    pub coin_x: Coin,     // in_coin
    pub x_amount_in: u64, // in_reserve

    pub coin_y: Coin,      // out_coin
    pub y_amount_out: u64, // out_au
}

#[derive(Deserialize)]
pub struct SwapEventRaw {
    pub x_in: String,
    pub x: CoinInfoRaw,
    pub y_out: String,
    pub y: CoinInfoRaw,
}

#[derive(Deserialize)]
pub struct CoinInfoRaw {
    pub account_address: String,
    pub module_name: String,
    pub struct_name: String,
}

impl ObricSwapEvent {
    //    {
    //      "guid": {
    //        "creation_number": "22",
    //        "account_address": "0xc7ea756470f72ae761b7986e4ed6fd409aad183b1b2d3d2f674d979852f45c4b"
    //      },
    //      "sequence_number": "2935",
    //      "type": "0xc7ea756470f72ae761b7986e4ed6fd409aad183b1b2d3d2f674d979852f45c4b::piece_swap::SwapEvent",
    //      "data": {
    //        "x": {
    //          "account_address": "0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea",
    //          "module_name": "0x636f696e",
    //          "struct_name": "0x54"
    //        },
    //        "x_in": "100000000",
    //        "y": {
    //          "account_address": "0x1",
    //          "module_name": "0x6170746f735f636f696e",
    //          "struct_name": "0x4170746f73436f696e"
    //        },
    //        "y_out": "2233909691"
    //      }
    //    },
    fn from_tag_and_data(_type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        //SwapEventDataRaw

        let raw_data: SwapEventRaw = serde_json::from_value(data)?;

        let coin_x = Coin::from_hex_strs(
            &raw_data.x.account_address,
            &raw_data.x.module_name,
            &raw_data.x.struct_name,
        )?;

        let coin_y = Coin::from_hex_strs(
            &raw_data.y.account_address,
            &raw_data.y.module_name,
            &raw_data.y.struct_name,
        )?;

        Ok(ObricSwapEvent {
            coin_x,                                     // in_coin
            x_amount_in: raw_data.x_in.parse::<u64>()?, // in_reserve

            coin_y,                                       // out_coin
            y_amount_out: raw_data.y_out.parse::<u64>()?, // out_au
        })
    }
}
