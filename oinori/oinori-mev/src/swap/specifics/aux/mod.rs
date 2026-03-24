pub mod event;
pub mod resource;

use aptos_api_types::Address as HexAddress;
use once_cell::sync::Lazy;
use std::str::FromStr;

pub const POOL_ADDRESS_STR: &str =
    "0xbd35135844473187163ca197ca93b2ab014370587bb0ed3befff9e902d6bb541";

const POOL_ADDRESS: Lazy<HexAddress> =
    Lazy::new(|| HexAddress::from_str(POOL_ADDRESS_STR).unwrap());
