use super::pool_type::*;
use super::weight::*;
use super::POOL_ADDRESS;
use crate::coins::*;
use crate::hasher::*;
use crate::resource::{
    coin_from_generics, get_value_from_struct_value, get_value_from_struct_with_type,
};
use crate::swap::amm::fixed_point64;
use crate::swap::{AmmCurveType, ConvertResource, ReservedCoin, ReservedCoinPair, ThalaAmmParam};
use anyhow::Result;
use aptos_api_types::{MoveStructTag, MoveStructValue, WriteSetChange};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
struct ThalaPair {
    x: CoinId,
    y: CoinId,
    stable: bool,
    weight_x: u64,
    weight_y: u64,
}
impl ThalaPair {
    fn new_stable(x: CoinId, y: CoinId) -> Self {
        ThalaPair {
            x,
            y,
            stable: true,
            weight_x: 0,
            weight_y: 0,
        }
    }

    fn new_weight(x: CoinId, y: CoinId, weight_x: u64, weight_y: u64) -> Self {
        ThalaPair {
            x,
            y,
            stable: false,
            weight_x,
            weight_y,
        }
    }
}

static THALA_SUPPORTED_PAIRS: Lazy<HashSet<ThalaPair>> = Lazy::new(|| {
    let mut supported_pairs: HashSet<ThalaPair> = HashSet::default();
    //--- stables --------------------------
    supported_pairs.insert(ThalaPair::new_stable(thala_mod_coin_id(), zusdc_coin_id()));
    supported_pairs.insert(ThalaPair::new_stable(thala_mod_coin_id(), zusdt_coin_id()));
    supported_pairs.insert(ThalaPair::new_stable(zusdc_coin_id(), zusdt_coin_id()));
    //
    //TODO(adjust)
    supported_pairs.insert(ThalaPair::new_stable(
        apt_coin_id(),
        tortuga_staked_aptos_coin_id(),
    ));
    supported_pairs.insert(ThalaPair::new_stable(
        tortuga_staked_aptos_coin_id(),
        apt_coin_id(),
    ));

    //--- weights --------------------------
    supported_pairs.insert(ThalaPair::new_weight(
        thala_mod_coin_id(),
        thala_coin_id(),
        20,
        80,
    ));

    supported_pairs.insert(ThalaPair::new_weight(
        thala_mod_coin_id(),
        apt_coin_id(),
        50,
        50,
    ));

    supported_pairs.insert(ThalaPair::new_weight(
        zusdc_coin_id(),
        zweth_coin_id(),
        50,
        50,
    ));

    supported_pairs.insert(ThalaPair::new_weight(
        thala_coin_id(),
        apt_coin_id(),
        50,
        50,
    ));

    //-    supported_coins.insert(apt_coin_id());
    //-    supported_coins.insert(thala_mod_coin_id());
    //-    supported_coins.insert(thala_coin_id());
    //-    supported_coins.insert(pan_cake_oft_coin_id());
    //-    supported_coins.insert(tortuga_staked_aptos_coin_id());
    //-    supported_coins.insert(whusdc_coin_id());
    //-    supported_coins.insert(whweth_coin_id());
    //-    supported_coins.insert(zusdc_coin_id());
    //-    supported_coins.insert(zusdt_coin_id());
    //-    supported_coins.insert(zweth_coin_id());

    supported_pairs
});

#[derive(Debug, Serialize, Clone)]
pub enum ThalaResource {
    Pool(ThalaPool),
}

const NULL_COIN: Lazy<CoinId> = Lazy::new(|| {
    CoinId::from_addresses_str(
        "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
        "base_pool",
        "Null",
    )
});

impl ConvertResource for ThalaResource {
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
                ("weighted_pool", "WeightedPool") => {
                    match ThalaPool::weight_pool_from_tag_and_data(type_tag, data)? {
                        None => Ok(None),
                        Some(weight_pool) => Ok(Some(ThalaResource::Pool(weight_pool))),
                    }
                },

                ("stable_pool", "StablePool") => {
                    match ThalaPool::stable_pool_from_tag_and_data(type_tag, data)? {
                        None => Ok(None),
                        Some(stable_pool) => Ok(Some(ThalaResource::Pool(stable_pool))),
                    }
                },

                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn into_coin_pair(self) -> Result<Option<ReservedCoinPair>> {
        let Self::Pool(ThalaPool {
            coin_x,
            coin_y,
            reserve_x,
            reserve_y,
            swap_fee_ratio,
            stable_info,
            pool_weight,
        }) = self;

        let coin_x_id = coin_x.as_coin_id().clone();
        let coin_y_id = coin_y.as_coin_id().clone();
        Ok(Some(ReservedCoinPair {
            x: ReservedCoin::new(coin_x, reserve_x),
            y: ReservedCoin::new(coin_y, reserve_y),
            fee_rate: 0,
            amount_scale: 0,
            amm_curve_type: AmmCurveType::ThalaAmm(ThalaAmmParam {
                coin_x_id,
                coin_y_id,
                pool_weight,
                stable_info,
                swap_fee_ratio,
            }),
            stable_scale_x: 0,
            stable_scale_y: 0,
            available: true,
        }))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThalaStableInfo {
    pub multiplexer_x: u64,
    pub multiplexer_y: u64,
    pub amp_factor: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThalaPool {
    pub coin_x: Coin,
    pub coin_y: Coin,
    pub reserve_x: u64,
    pub reserve_y: u64,

    pub swap_fee_ratio: fixed_point64::FixedPoint64,

    pub stable_info: Option<ThalaStableInfo>,
    pub pool_weight: Option<ThalaPoolWeight>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinValue {
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapFeeRatio {
    v: String,
}

impl ThalaPool {
    //      "data": {
    //        "type": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::weighted_pool::WeightedPool<0x6f986d146e4a90b828d8c12c14b6f4e003fdff11a8eecceceb63744363eaac01::mod_coin::MOD, 0x1::aptos_coin::AptosCoin, 0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::base_pool::Null, 0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::base_pool::Null, 0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::weighted_pool::Weight_50, 0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::weighted_pool::Weight_50, 0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::base_pool::Null, 0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::base_pool::Null>",
    //        "data": {
    //          "asset_0": {
    //            "value": "115592375727535"
    //          },
    //          "asset_1": {
    //            "value": "11574470990622"
    //          },
    //          "asset_2": {
    //            "value": "0"
    //          },
    //          "asset_3": {
    //            "value": "0"
    //          },
    //          "events": {
    //            "add_liquidity_events": {
    //              "counter": "372",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "42"
    //                }
    //              }
    //            },
    //            "param_change_events": {
    //              "counter": "0",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "45"
    //                }
    //              }
    //            },
    //            "pool_creation_events": {
    //              "counter": "1",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "41"
    //                }
    //              }
    //            },
    //            "remove_liquidity_events": {
    //              "counter": "104",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "43"
    //                }
    //              }
    //            },
    //            "swap_events": {
    //              "counter": "19266",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "44"
    //                }
    //              }
    //            }
    //          },
    //          "inverse_negated_swap_fee_ratio": {
    //            "v": "18502250826588749824"
    //          },
    //          "pool_token_burn_cap": {
    //            "dummy_field": false
    //          },
    //          "pool_token_mint_cap": {
    //            "dummy_field": false
    //          },
    //          "reserved_lp_coin": {
    //            "value": "100"
    //          },
    //          "swap_fee_ratio": {
    //            "v": "55340232221128654"
    //          },
    //          "weight_0": "50",
    //          "weight_1": "50",
    //          "weight_2": "0",
    //          "weight_3": "0"
    //        }
    //      }
    //
    fn weight_pool_from_tag_and_data(
        type_tag: &MoveStructTag,
        data: &MoveStructValue,
    ) -> Result<Option<Self>> {
        let weight_0 = get_value_from_struct_with_type!(data, "weight_0", String)?;
        let weight_1 = get_value_from_struct_with_type!(data, "weight_1", String)?;
        let weight_2 = get_value_from_struct_with_type!(data, "weight_2", String)?;
        let weight_3 = get_value_from_struct_with_type!(data, "weight_3", String)?;

        if weight_2.as_str() != "0" || weight_3.as_str() != "0" {
            return Ok(None);
        }

        let weight_0 = weight_0.parse::<u64>()?;
        let weight_1 = weight_1.parse::<u64>()?;

        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let pair = ThalaPair {
            x: coin_x.as_coin_id().clone(),
            y: coin_y.as_coin_id().clone(),
            stable: false,
            weight_x: weight_0,
            weight_y: weight_1,
        };

        if !THALA_SUPPORTED_PAIRS.contains(&pair) {
            return Ok(None);
        }

        let coin_z = coin_from_generics(&type_tag.generic_type_params, 2)?;
        let coin_z1 = coin_from_generics(&type_tag.generic_type_params, 3)?;

        if coin_z.as_coin_id() != &*NULL_COIN || coin_z1.as_coin_id() != &*NULL_COIN {
            return Ok(None);
        }

        let reserve_x = get_value_from_struct_with_type!(data, "asset_0", CoinValue)?;
        let reserve_y = get_value_from_struct_with_type!(data, "asset_1", CoinValue)?;

        let swap_fee_ratio =
            get_value_from_struct_with_type!(data, "swap_fee_ratio", SwapFeeRatio)?;

        let swap_fee_ratio = fixed_point64::FixedPoint64 {
            v: swap_fee_ratio.v.parse::<u128>()?,
        };

        let pool_weight = Some(ThalaPoolWeight::from_values(
            weight_0,
            weight_1,
            weight_2.parse::<u64>()?,
            weight_3.parse::<u64>()?,
        )?);

        Ok(Some(ThalaPool {
            coin_x,
            coin_y,

            reserve_x: reserve_x.value.parse::<u64>()?,
            reserve_y: reserve_y.value.parse::<u64>()?,

            swap_fee_ratio,

            stable_info: None,
            pool_weight,
        }))
    }

    //    {
    //      "type": "write_resource",
    //      "address": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //      "state_key_hash": "0x29575f4265c800451472c6b570c8e8fdc4b5a4ac2cb8a82b4a3d4a348da2208c",
    //      "data": {
    //        "type": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::stable_pool::StablePool<0x6f986d146e4a90b828d8c12c14b6f4e003fdff11a8eecceceb63744363eaac01::mod_coin::MOD, 0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa::asset::USDC, 0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::base_pool::Null, 0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::base_pool::Null>",
    //        "data": {
    //          "amp_factor": "100",
    //          "asset_0": {
    //            "value": "218312770039817"
    //          },
    //          "asset_1": {
    //            "value": "5069138291484"
    //          },
    //          "asset_2": {
    //            "value": "0"
    //          },
    //          "asset_3": {
    //            "value": "0"
    //          },
    //          "events": {
    //            "add_liquidity_events": {
    //              "counter": "335",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "37"
    //                }
    //              }
    //            },
    //            "param_change_events": {
    //              "counter": "0",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "40"
    //                }
    //              }
    //            },
    //            "pool_creation_events": {
    //              "counter": "1",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "36"
    //                }
    //              }
    //            },
    //            "remove_liquidity_events": {
    //              "counter": "145",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "38"
    //                }
    //              }
    //            },
    //            "swap_events": {
    //              "counter": "22261",
    //              "guid": {
    //                "id": {
    //                  "addr": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af",
    //                  "creation_num": "39"
    //                }
    //              }
    //            }
    //          },
    //          "inverse_negated_swap_fee_ratio": {
    //            "v": "18465209284871323648"
    //          },
    //          "pool_token_burn_cap": {
    //            "dummy_field": false
    //          },
    //          "pool_token_mint_cap": {
    //            "dummy_field": false
    //          },
    //          "precision_multipliers": ["1", "100"],
    //          "reserved_lp_coin": {
    //            "value": "100"
    //          },
    //          "swap_fee_ratio": {
    //            "v": "18446744073709551"
    //          }
    //        }
    //      }
    //    },
    //

    fn stable_pool_from_tag_and_data(
        type_tag: &MoveStructTag,
        data: &MoveStructValue,
    ) -> Result<Option<Self>> {
        //        "type": "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::stable_pool::StablePool<
        //        0x6f986d146e4a90b828d8c12c14b6f4e003fdff11a8eecceceb63744363eaac01::mod_coin::MOD,
        //        0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa::asset::USDC,
        //        0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::base_pool::Null,
        //        0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af::base_pool::Null>",

        let coin_x = coin_from_generics(&type_tag.generic_type_params, 0)?;
        let coin_y = coin_from_generics(&type_tag.generic_type_params, 1)?;

        let pair = ThalaPair {
            x: coin_x.as_coin_id().clone(),
            y: coin_y.as_coin_id().clone(),
            stable: true,
            weight_x: 0,
            weight_y: 0,
        };

        if !THALA_SUPPORTED_PAIRS.contains(&pair) {
            return Ok(None);
        }

        // ignore   stable pool of not a stable coin (e.g, APT / TAPT pair will be ignored)
        if !coin_x.is_stable() || !coin_y.is_stable() {
            return Ok(None);
        }

        let coin_z = coin_from_generics(&type_tag.generic_type_params, 2)?;
        let coin_z1 = coin_from_generics(&type_tag.generic_type_params, 3)?;

        if coin_z.as_coin_id() != &*NULL_COIN || coin_z1.as_coin_id() != &*NULL_COIN {
            return Ok(None);
        }

        let reserve_x = get_value_from_struct_with_type!(data, "asset_0", CoinValue)?;
        let reserve_y = get_value_from_struct_with_type!(data, "asset_1", CoinValue)?;
        let amp_factor = get_value_from_struct_with_type!(data, "amp_factor", String)?;

        // precision_multiplexer_str  ==["1", "100"]
        let stable_precision_multiplexers =
            get_value_from_struct_with_type!(data, "precision_multipliers", Vec<String>)?;
        let stalbe_multiplexer_x = stable_precision_multiplexers
            .get(0)
            .unwrap()
            .parse::<u64>()?;
        let stalbe_multiplexer_y = stable_precision_multiplexers
            .get(1)
            .unwrap()
            .parse::<u64>()?;

        let stable_info = ThalaStableInfo {
            multiplexer_x: stalbe_multiplexer_x,
            multiplexer_y: stalbe_multiplexer_y,
            amp_factor: amp_factor.parse::<u64>()?,
        };

        let swap_fee_ratio =
            get_value_from_struct_with_type!(data, "swap_fee_ratio", SwapFeeRatio)?;

        let swap_fee_ratio = fixed_point64::FixedPoint64 {
            v: swap_fee_ratio.v.parse::<u128>()?,
        };

        Ok(Some(ThalaPool {
            coin_x,
            coin_y,

            reserve_x: reserve_x.value.parse::<u64>()?,
            reserve_y: reserve_y.value.parse::<u64>()?,
            stable_info: Some(stable_info),

            swap_fee_ratio,
            pool_weight: None,
        }))
    }
}
