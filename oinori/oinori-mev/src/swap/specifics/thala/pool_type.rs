use super::weight::*;
use crate::coins::*;
use anyhow::{anyhow, Result};

const BITS_MASK_COIN: u64 = 0x7ff;
const BITS_MASK_WEIGHT: u64 = 0x1f;

const COIN_1_SHIFT: u64 = 53;
const COIN_2_SHIFT: u64 = 42;
const COIN_3_SHIFT: u64 = 31;
const COIN_4_SHIFT: u64 = 20;

const WEIGHT_1_SHIFT: u64 = 15;
const WEIGHT_2_SHIFT: u64 = 10;
const WEIGHT_3_SHIFT: u64 = 5;

#[derive(Debug)]
pub struct ThalaPoolType {
    pub coin_x: CoinId,
    pub coin_y: CoinId,

    pub coin_z: Option<CoinId>,
    pub coin_z1: Option<CoinId>,
    pub weight: Option<ThalaPoolWeight>,
}

impl ThalaPoolType {
    pub fn new_with_pair_coins(
        coin_x: CoinId,
        coin_y: CoinId,
        weight: Option<ThalaPoolWeight>,
    ) -> Self {
        Self {
            coin_x,
            coin_y,
            coin_z: None,
            coin_z1: None,
            weight,
        }
    }

    pub fn encode(&self) -> Result<u64> {
        let coin_1 = encode_thala_coin_type(Some(&self.coin_x))?;
        let coin_2 = encode_thala_coin_type(Some(&self.coin_y))?;
        let coin_3 = encode_thala_coin_type(self.coin_z.as_ref())?;
        let coin_4 = encode_thala_coin_type(self.coin_z1.as_ref())?;

        let (w1, w2, w3, w4) = match &self.weight {
            None => (0, 0, 0, 0),
            Some(weight) => weight.unpack_encoded_weights(),
        };
        let encoded_value: u64 = ((coin_1 as u64) << COIN_1_SHIFT)
            | ((coin_2 as u64) << COIN_2_SHIFT)
            | ((coin_3 as u64) << COIN_3_SHIFT)
            | ((coin_4 as u64) << COIN_4_SHIFT)
            | (w1 << WEIGHT_1_SHIFT)
            | (w2 << WEIGHT_2_SHIFT)
            | (w3 << WEIGHT_3_SHIFT)
            | w4;

        Ok(encoded_value)
    }

    pub fn decode(encoded_pool_type: u64) -> Result<Self> {
        let coin_1 = (encoded_pool_type >> COIN_1_SHIFT) & BITS_MASK_COIN;
        let coin_1 = decode_thala_coin_type(coin_1 as u16)?
            .ok_or(anyhow!("coin_1 must no be null {}", coin_1))?;

        let coin_2 = (encoded_pool_type >> COIN_2_SHIFT) & BITS_MASK_COIN;
        let coin_2 = decode_thala_coin_type(coin_2 as u16)?
            .ok_or(anyhow!("coin_2 must no be null {}", coin_2))?;

        let coin_3 = (encoded_pool_type >> COIN_3_SHIFT) & BITS_MASK_COIN;
        let coin_3 = decode_thala_coin_type(coin_3 as u16)?;

        let coin_4 = (encoded_pool_type >> COIN_4_SHIFT) & BITS_MASK_COIN;
        let coin_4 = decode_thala_coin_type(coin_4 as u16)?;

        let w1 = (encoded_pool_type >> WEIGHT_1_SHIFT) & BITS_MASK_WEIGHT;
        let w2 = (encoded_pool_type >> WEIGHT_2_SHIFT) & BITS_MASK_WEIGHT;
        let w3 = (encoded_pool_type >> WEIGHT_3_SHIFT) & BITS_MASK_WEIGHT;
        let w4 = encoded_pool_type & BITS_MASK_WEIGHT;

        let weight = ThalaPoolWeight::from_encoded_weights(w1, w2, w3, w4)?;
        Ok(ThalaPoolType {
            coin_x: coin_1,
            coin_y: coin_2,
            coin_z: coin_3,
            coin_z1: coin_4,
            weight,
        })
    }
}

pub fn encode_thala_coin_type(coin_id: Option<&CoinId>) -> Result<u16> {
    let v = match coin_id {
        None => 0,
        Some(coin_id) => {
            if coin_id == &apt_coin_id() {
                1
            } else if coin_id == &thala_mod_coin_id() {
                2
            } else if coin_id == &thala_coin_id() {
                3
            } else if coin_id == &pan_cake_oft_coin_id() {
                4
            } else if coin_id == &tortuga_staked_aptos_coin_id() {
                5
            } else if coin_id == &whusdc_coin_id() {
                6
            } else if coin_id == &whweth_coin_id() {
                7
            } else if coin_id == &zusdc_coin_id() {
                8
            } else if coin_id == &zusdt_coin_id() {
                9
            } else if coin_id == &zweth_coin_id() {
                10
            } else {
                return Err(anyhow!("Unsupported coin type {:?}", coin_id));
            }
        },
    };

    Ok(v)
}

pub fn decode_thala_coin_type(coin_type: u16) -> Result<Option<CoinId>> {
    let v = match coin_type {
        0 => None,
        1 => Some(apt_coin_id()),
        2 => Some(thala_mod_coin_id()),
        3 => Some(thala_coin_id()),
        4 => Some(pan_cake_oft_coin_id()),
        5 => Some(tortuga_staked_aptos_coin_id()),
        6 => Some(whusdc_coin_id()),
        7 => Some(whweth_coin_id()),
        8 => Some(zusdc_coin_id()),
        9 => Some(zusdt_coin_id()),
        10 => Some(zweth_coin_id()),
        other => return Err(anyhow!("Unsupported coin type {:?}", other)),
    };
    Ok(v)
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::coins::*;

    #[test]
    fn test_thala_coin_type_encode() {
        assert_eq!(encode_thala_coin_type(None).unwrap(), 0u16);
        assert_eq!(
            encode_thala_coin_type(Some(&tortuga_staked_aptos_coin_id())).unwrap(),
            5u16
        );

        assert!(encode_thala_coin_type(Some(&aptoge_coin_id())).is_err());

        // decode
        assert!(decode_thala_coin_type(0).unwrap().is_none());
        assert!(decode_thala_coin_type(11).is_err());

        for each_coin_type in 1..=10 {
            let coin_id = decode_thala_coin_type(each_coin_type).unwrap().unwrap();
            let encoded_coin_type = encode_thala_coin_type(Some(&coin_id)).unwrap();
            assert_eq!(each_coin_type, encoded_coin_type);
        }
    }

    #[test]
    fn test_thala_pool_type_encode_decode_weight_pool_1() {
        let pool_type_value = 18018796556331008;
        let pool_type = ThalaPoolType::decode(pool_type_value).unwrap();

        {
            let encoded = pool_type.encode().unwrap();
            assert_eq!(pool_type_value, encoded);
        }

        assert_eq!(pool_type.coin_x, thala_mod_coin_id());
        assert_eq!(pool_type.coin_y, apt_coin_id());

        assert_eq!(pool_type.coin_z, None);
        assert_eq!(pool_type.coin_z1, None);
        let weight = pool_type.weight.unwrap();

        assert_eq!(weight.w1, ThalaWeight(50));
        assert_eq!(weight.w2, ThalaWeight(50));
        assert_eq!(weight.w3, None);
        assert_eq!(weight.w4, None);
    }

    #[test]
    fn test_thala_pool_type_encode_decode_stable_pool_2() {
        let pool_type_value = 18049582881570816;
        let pool_type = ThalaPoolType::decode(pool_type_value).unwrap();

        {
            let encoded = pool_type.encode().unwrap();
            assert_eq!(pool_type_value, encoded);
        }

        assert_eq!(pool_type.coin_x, thala_mod_coin_id());
        assert_eq!(pool_type.coin_y, zusdc_coin_id());

        assert_eq!(pool_type.coin_z, None);
        assert_eq!(pool_type.coin_z1, None);

        assert!(pool_type.weight.is_none());
    }
}
