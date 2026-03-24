mod curve_stable;
mod liquid_stable;
mod obric_amm;
mod thala_amm;
mod uniswap_v2;

use crate::coins::{Coin, CoinId};
use anyhow::Result;
pub use obric_amm::ObricAmmParam;
use serde::{Deserialize, Serialize};
use static_init::dynamic;
pub use thala_amm::*;
use uint::construct_uint;

construct_uint! {
    pub struct U256(4);
}

// 10*8
#[dynamic]
pub static EIGHT_FIG_NUM: U256 = U256::try_from(100_000_000_u64).unwrap();

#[dynamic]
pub static CONST_ZERO: U256 = U256::try_from(0_u64).unwrap();

#[dynamic]
pub static CONST_ONE: U256 = U256::try_from(1_u64).unwrap();

#[dynamic]
pub static CONST_TWO: U256 = U256::try_from(2_u64).unwrap();

#[dynamic]
pub static CONST_THREE: U256 = U256::try_from(3_u64).unwrap();

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CoinSwapDirection {
    XToY,
    YToX,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum AmmCurveType {
    LiquidStable,
    CurveStableWithAdditionalFee(AdditionalFee),
    UniswapV2,                                 // x * y = k
    UniswapV2WithAdditionalFee(AdditionalFee), // x * y = k
    ObricAmm(ObricAmmParam),
    ThalaAmm(ThalaAmmParam),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "additional_fee_type", content = "additional_fee_rate")]
pub enum AdditionalFee {
    FromX(u64),
    FromY(u64),
}

#[derive(Debug, Serialize)]
pub struct SwapPairInfo<'a> {
    pub in_coin: &'a Coin,
    pub out_coin: &'a Coin,
    pub amm_curve_type: &'a AmmCurveType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReservedCoinPair {
    pub x: ReservedCoin,
    pub y: ReservedCoin,
    pub fee_rate: u64,
    pub amount_scale: u64,
    pub amm_curve_type: AmmCurveType,
    pub stable_scale_x: u64,
    pub stable_scale_y: u64,
    pub available: bool,
}

impl ReservedCoinPair {
    pub fn coin_ids(&self) -> (&CoinId, &CoinId) {
        (self.x.coin.as_coin_id(), self.y.coin.as_coin_id())
    }

    pub fn has_empty_reserved_amount(&self) -> bool {
        self.x.empty_reserved_amount() || self.x.empty_reserved_amount()
    }
    pub fn is_stable_coin_pair(&self) -> bool {
        self.x.coin.stable_coin && self.y.coin.stable_coin
    }

    pub fn as_swap_pair_info<'a>(&'a self, direction: CoinSwapDirection) -> SwapPairInfo<'a> {
        let (in_coin, out_coin) = match direction {
            CoinSwapDirection::XToY => (&self.x.coin, &self.y.coin),
            CoinSwapDirection::YToX => (&self.y.coin, &self.x.coin),
        };

        SwapPairInfo {
            in_coin,
            out_coin,
            amm_curve_type: &self.amm_curve_type,
        }
    }

    // if fee_scale = 10000 (like liquid swap) and fee_rate is 30
    // the fee is  30/10000
    pub fn calc_amount_out(
        &self,
        in_amount: u64,
        swap_direction: &CoinSwapDirection,
    ) -> Result<u64> {
        let (reserve_in, reserve_out, stable_scale_in, stable_scale_out) = match swap_direction {
            CoinSwapDirection::XToY => (
                self.x.reserverd_amount,
                self.y.reserverd_amount,
                self.stable_scale_x,
                self.stable_scale_y,
            ),

            CoinSwapDirection::YToX => (
                self.y.reserverd_amount,
                self.x.reserverd_amount,
                self.stable_scale_y,
                self.stable_scale_x,
            ),
        };

        match &self.amm_curve_type {
            AmmCurveType::LiquidStable => {
                liquid_stable::get_liquid_stable_coin_out_with_fee(
                    in_amount,
                    reserve_in,
                    reserve_out,
                    self.fee_rate,
                    self.amount_scale,
                    stable_scale_in,
                    stable_scale_out,
                )
            },
            AmmCurveType::UniswapV2 => Ok(uniswap_v2::get_uniswap_v2_coin_out_with_fee(
                in_amount,
                reserve_in,
                reserve_out,
                self.fee_rate,
                self.amount_scale,
            )),

            AmmCurveType::ThalaAmm(thala_amm_param) => {
                Ok(thala_amm::get_thala_swap_coin_out_with_fee(
                    in_amount,
                    reserve_in,
                    reserve_out,
                    thala_amm_param,
                    swap_direction,
                ))
            },

            AmmCurveType::ObricAmm(obric_amm_param) => {
                Ok(obric_amm::get_obric_swap_coin_out_with_fee(
                    in_amount,
                    reserve_in,
                    reserve_out,
                    self.fee_rate,
                    self.amount_scale,
                    obric_amm_param,
                    swap_direction,
                )?)
            },

            AmmCurveType::UniswapV2WithAdditionalFee(addtional_fee) => {
                let (fee_from_in_amount, fee_from_out_amount) = match swap_direction {
                    CoinSwapDirection::XToY => match addtional_fee {
                        AdditionalFee::FromX(x_fee) => (*x_fee, 0u64),
                        AdditionalFee::FromY(y_fee) => (0u64, *y_fee),
                    },

                    CoinSwapDirection::YToX => match addtional_fee {
                        AdditionalFee::FromX(x_fee) => (0u64, *x_fee),
                        AdditionalFee::FromY(y_fee) => (*y_fee, 0u64),
                    },
                };

                let out_amount = uniswap_v2::get_uniswap_v2_coin_out_with_fee(
                    amount_after_fee(in_amount, fee_from_in_amount, self.amount_scale),
                    reserve_in,
                    reserve_out,
                    self.fee_rate,
                    self.amount_scale,
                );

                Ok(amount_after_fee(
                    out_amount,
                    fee_from_out_amount,
                    self.amount_scale,
                ))
            },

            AmmCurveType::CurveStableWithAdditionalFee(_addtional_fee) => {
                unimplemented!("curve stable is not available yet")
            },
        }
    }

    pub fn update_new_reserve(&mut self, x: u64, y: u64) {
        self.x.reserverd_amount = x;
        self.y.reserverd_amount = y;
    }
}

fn amount_after_fee(amount: u64, fee_rate: u64, amount_scale: u64) -> u64 {
    if fee_rate == 0 {
        amount
    } else {
        ((amount as u128 * amount_scale as u128 - amount as u128 * fee_rate as u128)
            / amount_scale as u128) as u64
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReservedCoin {
    pub coin: Coin,
    pub reserverd_amount: u64,
}

impl ReservedCoin {
    pub fn as_coin_id(&self) -> &CoinId {
        &self.coin.coin_id
    }
    pub fn new(coin: Coin, reserverd_amount: u64) -> Self {
        Self {
            coin,
            reserverd_amount,
        }
    }
    pub fn empty_reserved_amount(&self) -> bool {
        self.reserverd_amount == 0
    }
}
