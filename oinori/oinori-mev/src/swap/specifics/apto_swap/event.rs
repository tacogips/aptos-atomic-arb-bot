use anyhow::Result;
use aptos_api_types::{Event, MoveStructTag, MoveType};

use serde::{Deserialize, Serialize};

use super::POOL_ADDRESS;

#[derive(Debug, Serialize)]
pub enum AptoSwapEvent {
    SwapEvent(AptoSwapSwapEvent),
}

impl AptoSwapEvent {
    pub fn parse(e: &Event) -> Result<Option<Self>> {
        if e.guid.account_address != *POOL_ADDRESS {
            Ok(None)
        } else if let MoveType::Struct(struct_tag) = &e.typ {
            if struct_tag.address == *POOL_ADDRESS {
                match (struct_tag.module.as_str(), struct_tag.name.as_str()) {
                    ("pool", "SwapTokenEvent") => Ok(Some(AptoSwapEvent::SwapEvent(
                        AptoSwapSwapEvent::from_tag_and_data(struct_tag, e.data.clone())?,
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
pub struct AptoSwapSwapEvent {
    pub in_amount: u64,  // in_reserve
    pub out_amount: u64, // out_au
    pub x_to_y: bool,
}

#[derive(Deserialize)]
pub struct SwapEventRaw {
    pub in_amount: String,  // in_reserve
    pub out_amount: String, // out_au
    pub x_to_y: bool,
}

impl AptoSwapSwapEvent {
    //    {
    //      "guid": {
    //        "creation_number": "28",
    //        "account_address": "0xa5d3ac4d429052674ed38adc62d010e52d7c24ca159194d17ddc196ddb7e480b"
    //      },
    //      "sequence_number": "13841",
    //      "type": "0xa5d3ac4d429052674ed38adc62d010e52d7c24ca159194d17ddc196ddb7e480b::pool::SwapTokenEvent",
    //      "data": {
    //        "in_amount": "4704986",
    //        "out_amount": "99963385",
    //        "x_to_y": false
    //      }
    //    }
    fn from_tag_and_data(_type_tag: &MoveStructTag, data: serde_json::Value) -> Result<Self> {
        let SwapEventRaw {
            in_amount,
            out_amount,
            x_to_y,
        } = serde_json::from_value(data)?;

        Ok(AptoSwapSwapEvent {
            in_amount: in_amount.parse::<u64>()?,
            out_amount: out_amount.parse::<u64>()?, // out_au
            x_to_y,
        })
    }
}
