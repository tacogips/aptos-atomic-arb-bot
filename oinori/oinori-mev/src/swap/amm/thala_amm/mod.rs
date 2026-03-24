#![allow(clippy::style)]
#![allow(non_snake_case)]
#![allow(unused_parens)]
#![allow(unused_mut)]
#![allow(unused_assignments)]

pub mod fixed_point64;
mod thala_stable_swap;
mod thala_weighted_swap;

use crate::coins::CoinId;
use crate::swap::amm::CoinSwapDirection;
use crate::swap::specifics::thala::{ThalaPoolType, ThalaPoolWeight, ThalaStableInfo, ThalaWeight};
use anyhow::Result;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThalaAmmParam {
    pub coin_x_id: CoinId,
    pub coin_y_id: CoinId,

    pub swap_fee_ratio: fixed_point64::FixedPoint64,
    pub pool_weight: Option<ThalaPoolWeight>,
    pub stable_info: Option<ThalaStableInfo>,
}

impl ThalaAmmParam {
    pub fn as_hippo_pool_type(&self) -> u64 {
        // the pool type will be 0 if an unknown coin pool is added
        ThalaPoolType::new_with_pair_coins(
            self.coin_x_id.clone(),
            self.coin_y_id.clone(),
            self.pool_weight.clone(),
        )
        .encode()
        .unwrap_or(0)
    }
}

pub fn get_thala_swap_coin_out_with_fee(
    coin_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    param: &ThalaAmmParam,
    swap_direction: &CoinSwapDirection,
) -> u64 {
    match (param.pool_weight.as_ref(), param.stable_info.as_ref()) {
        (_, Some(stable_info)) => {
            // calc out amount from weighted  pool
            let (multiplexer_in, multiplexer_out) = match swap_direction {
                CoinSwapDirection::XToY => (stable_info.multiplexer_x, stable_info.multiplexer_y),
                CoinSwapDirection::YToX => (stable_info.multiplexer_y, stable_info.multiplexer_x),
            };

            match stable_swap_exact_in(
                coin_in,
                reserve_in,
                reserve_out,
                multiplexer_in,
                multiplexer_out,
                stable_info.amp_factor,
                param.swap_fee_ratio,
            ) {
                Ok(out_amount) => out_amount,
                Err(_) => 0,
            }
        },

        (Some(pool_weight), _) => {
            // calc out amount from weighted  pool
            let (weight_in, weight_out) = match swap_direction {
                CoinSwapDirection::XToY => (pool_weight.w1, pool_weight.w2),
                CoinSwapDirection::YToX => (pool_weight.w2, pool_weight.w1),
            };

            //println!(
            //    "======coin in {},resrve_in:{},reserve:out:{}, param{:?}, direction:{:?}",
            //    coin_in, reserve_in, reserve_out, param, swap_direction,
            //); // TODO debug

            weight_swap_exact_in(
                coin_in,
                reserve_in,
                reserve_out,
                param.swap_fee_ratio,
                weight_in,
                weight_out,
            )
        },

        _ => {
            panic!("invalid thala param :{:?}", param)
        },
    }
}

fn calc_fee(coin_in: u64, swap_fee_ratio: fixed_point64::FixedPoint64) -> u64 {
    let total_fee_amount = fixed_point64::decode(fixed_point64::mul(swap_fee_ratio, coin_in));
    total_fee_amount
}

fn stable_swap_exact_in(
    coin_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    multiplexer_in: u64,
    multiplexer_out: u64,
    amp_factor: u64,
    swap_fee_ratio: fixed_point64::FixedPoint64,
) -> Result<u64> {
    let total_fee_amount = calc_fee(coin_in, swap_fee_ratio);
    let amount_in_post_fee = coin_in - total_fee_amount;

    let balance_in = reserve_in * multiplexer_in;
    let balance_out = reserve_out * multiplexer_out;

    let calc_in = amount_in_post_fee * multiplexer_in;
    let calc_out =
        thala_stable_swap::calc_out_given_in(amp_factor, calc_in, balance_in, balance_out)?;

    let amount_out = calc_out / multiplexer_out;

    Ok(amount_out)
}

fn weight_swap_exact_in(
    coin_in: u64,
    balance_in: u64,
    balance_out: u64,
    swap_fee_ratio: fixed_point64::FixedPoint64,

    weight_in: ThalaWeight,
    weight_out: ThalaWeight,
) -> u64 {
    let amount_in = coin_in;
    let total_fee_amount = calc_fee(coin_in, swap_fee_ratio);

    let amount_in_post_fee = amount_in - total_fee_amount;

    let amount_out = thala_weighted_swap::calc_out_given_in_weights_u64(
        amount_in_post_fee,
        balance_in,
        balance_out,
        weight_in.value(),
        weight_out.value(),
    );
    amount_out
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::*;

    #[test]
    fn test_thala_thl_in_mod_out_weight_80_20() {
        let coin_in = 300000000u64;
        let reserve_in = 507941296652919u64;
        let reserve_out = 39743926375303u64;

        let json = json!(
         {
          "coin_x_id": "0x6f986d146e4a90b828d8c12c14b6f4e003fdff11a8eecceceb63744363eaac01::mod_coin::MOD",
          "coin_y_id": "0x7fd500c11216f0fe3095d0c4b8aa4d64a4e2e04f83758462f2b127255643615::thl_coin::THL",
          "swap_fee_ratio": {
            "v": "55340232221128654",
          },
          "pool_weight": {
            "w1": 20,
            "w2": 80,
            "w3": null,
            "w4": null
          },
          "stable_info": null
        });
        let amm_param = serde_json::from_value::<ThalaAmmParam>(json).unwrap();

        assert_eq!(amm_param.as_hippo_pool_type(), 18027592649162752);

        let actual_out = get_thala_swap_coin_out_with_fee(
            coin_in,
            reserve_in,
            reserve_out,
            &amm_param,
            &CoinSwapDirection::YToX,
        );

        let expected_out = 93612320u64;
        assert_eq!(actual_out, expected_out);
    }

    #[test]
    fn test_thala_calc_amount_overflow_1() {
        let coin_in = 39u64;
        let reserve_in = 39787602475408;
        let reserve_out = 507801507823408;

        let json = json!(
        {
          "coin_x_id": "0x6f986d146e4a90b828d8c12c14b6f4e003fdff11a8eecceceb63744363eaac01::mod_coin::MOD",
          "coin_y_id": "0x7fd500c11216f0fe3095d0c4b8aa4d64a4e2e04f83758462f2b127255643615::thl_coin::THL",
          "swap_fee_ratio":  { "v": "55340232221128654" },
          "pool_weight": {
              "w1": 20,
              "w2": 80,
              "w3": null,
              "w4": null
          },
          "stable_info": null
        });
        let amm_param = serde_json::from_value::<ThalaAmmParam>(json).unwrap();
        assert_eq!(amm_param.as_hippo_pool_type(), 18027592649162752);

        let actual_out = get_thala_swap_coin_out_with_fee(
            coin_in,
            reserve_in,
            reserve_out,
            &amm_param,
            &CoinSwapDirection::XToY,
        );

        let expected_out = 0;
        assert_eq!(actual_out, expected_out);
    }
}

