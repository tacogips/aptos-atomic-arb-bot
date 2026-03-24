use super::POOL_ADDRESS;
use crate::coins::Coin;
use crate::resource::{
    coin_from_generics, get_value_from_struct_value, get_value_from_struct_with_type,
};
use crate::swap::ConvertResource;
use crate::swap::{AmmCurveType, ObricAmmParam, ReservedCoin, ReservedCoinPair};
use anyhow::Result;
use aptos_api_types::{MoveStructTag, MoveStructValue, WriteSetChange};
use serde::{Deserialize, Serialize};

const AMOUNT_SCALE: u64 = 1_000_000;

#[derive(Debug, Serialize, Clone)]
pub enum ObricResource {
    Pool(ObricPool),
}

impl ConvertResource for ObricResource {
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
                ("piece_swap", "PieceSwapPoolInfo") => Ok(Some(ObricResource::Pool(
                    ObricPool::from_tag_and_data(type_tag, data)?,
                ))),

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let Self::Pool(ObricPool {
            coin_x,
            coin_y,
            reserve_x,
            reserve_y,
            swap_fee_per_million,
            k,
            k2,
            m,
            n,
            xa,
            xb,
            x_deci_mult,
            y_deci_mult,
        }) = self;

        Ok(Some(ReservedCoinPair {
            x: ReservedCoin::new(coin_x, reserve_x),
            y: ReservedCoin::new(coin_y, reserve_y),
            fee_rate: swap_fee_per_million,
            amount_scale: AMOUNT_SCALE,
            amm_curve_type: AmmCurveType::ObricAmm(ObricAmmParam {
                k,
                k2,
                m,
                n,
                xa,
                xb,
                x_deci_mult,
                y_deci_mult,
            }),
            stable_scale_x: 0,
            stable_scale_y: 0,
            available: true,
        }))
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ObricPool {
    pub coin_x: Coin,
    pub coin_y: Coin,
    pub reserve_x: u64,
    pub reserve_y: u64,

    pub swap_fee_per_million: u64,

    pub k: u128,
    pub k2: u128,
    pub xa: u64,
    pub xb: u64,

    pub x_deci_mult: u64,
    pub y_deci_mult: u64,

    pub m: u64,
    pub n: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinValue {
    value: String,
}

impl ObricPool {
    //      "data": {
    //        "type": "0xc7ea756470f72ae761b7986e4ed6fd409aad183b1b2d3d2f674d979852f45c4b::piece_swap::PieceSwapPoolInfo<0x5e156f1207d0ebfa19a9eeff00d62a282278fb8719f4fab3a586a0a2c0fffbea::coin::T, 0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa::asset::USDC>",
    //        "data": {
    //          "K": "1000000000000000000",
    //          "K2": "2195873881596",
    //          "Xa": "1474494",
    //          "Xb": "11424866",
    //          "lp_amt": "9095542797",
    //          "m": "993562696",
    //          "n": "9935628",
    //          "protocol_fee_share_per_thousand": "300",
    //          "protocol_fee_x": {
    //            "value": "32415993"
    //          },
    //          "protocol_fee_y": {
    //            "value": "32608936"
    //          },
    //          "reserve_x": {
    //            "value": "10156806617"
    //          },
    //          "reserve_y": {
    //            "value": "8229588447"
    //          },
    //          "swap_fee_per_million": "200",
    //          "x_deci_mult": "1001",
    //          "y_deci_mult": "1000"
    //        }
    //      }
    fn from_tag_and_data(type_tag: &MoveStructTag, data: &MoveStructValue) -> Result<Self> {
        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let reserve_x = get_value_from_struct_with_type!(data, "reserve_x", CoinValue)?;
        let reserve_y = get_value_from_struct_with_type!(data, "reserve_y", CoinValue)?;

        let swap_fee_per_million =
            get_value_from_struct_with_type!(data, "swap_fee_per_million", String)?;

        let k = get_value_from_struct_with_type!(data, "K", String)?;
        let k2 = get_value_from_struct_with_type!(data, "K2", String)?;

        let xa = get_value_from_struct_with_type!(data, "Xa", String)?;
        let xb = get_value_from_struct_with_type!(data, "Xb", String)?;

        let x_deci_mult = get_value_from_struct_with_type!(data, "x_deci_mult", String)?;
        let y_deci_mult = get_value_from_struct_with_type!(data, "y_deci_mult", String)?;

        let m = get_value_from_struct_with_type!(data, "m", String)?;
        let n = get_value_from_struct_with_type!(data, "n", String)?;
        Ok(ObricPool {
            coin_x,
            coin_y,

            reserve_x: reserve_x.value.parse::<u64>()?,
            reserve_y: reserve_y.value.parse::<u64>()?,

            swap_fee_per_million: swap_fee_per_million.parse::<u64>()?,

            k: k.parse::<u128>()?,
            k2: k2.parse::<u128>()?,
            xa: xa.parse::<u64>()?,
            xb: xb.parse::<u64>()?,

            x_deci_mult: x_deci_mult.parse::<u64>()?,
            y_deci_mult: y_deci_mult.parse::<u64>()?,

            m: m.parse::<u64>()?,
            n: n.parse::<u64>()?,
        })
    }
}
