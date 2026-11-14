pub mod event;
pub mod resource;

use aptos_api_types::Address as HexAddress;
use once_cell::sync::Lazy;
use std::str::FromStr;

pub const POOL_ADDRESS_STR: &str =
    "0xa5d3ac4d429052674ed38adc62d010e52d7c24ca159194d17ddc196ddb7e480b";

const POOL_ADDRESS: Lazy<HexAddress> =
    Lazy::new(|| HexAddress::from_str(POOL_ADDRESS_STR).unwrap());