mod apt_coin;

use crate::account_address::hex_to_account_address;
use crate::hasher::*;
use crate::read_db;
use crate::resource::struct_tag_from_resource_without_generics;
use anyhow::{anyhow, Error as AnyhowError, Result};
pub use apt_coin::*;
use aptos_api_types::{
    Address, IdentifierWrapper, MoveConverter, MoveResource, MoveStructTag, MoveType,
};
use aptos_storage_interface::DbReader;
use aptos_types::account_address::AccountAddress;
use aptos_vm::data_cache::AsMoveResolver;
use move_core_types::identifier::Identifier;
use num_integer::Integer;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

use aptos_storage_interface::state_view::DbStateView;

pub static REGISTERED_COINS: Lazy<RwLock<CoinPool>> =
    Lazy::new(|| RwLock::new(CoinPool::load().unwrap()));

pub static COIN_AMOUNT_IDENTIFIER: Lazy<IdentifierWrapper> =
    Lazy::new(|| IdentifierWrapper(Identifier::new("coin").unwrap()));

//TODO add coin
const COIN_RESOURCE_TAG: &str = "0x1::coin::CoinStore";

pub fn pan_cake_oft_coin_id() -> CoinId {
    //coin_id = "0x159df6b7689437016108a019fd5bef736bac692b6d4a1f10c941f6fbb9a74ca6::oft::CakeOFT"
    CoinId::from_addresses_str(
        "0x159df6b7689437016108a019fd5bef736bac692b6d4a1f10c941f6fbb9a74ca6",
        "oft",
        "CakeOFT",
    )
}

pub fn whbtc_coin_id() -> CoinId {
    //0xae478ff7d83ed072dbc5e264250e67ef58f57c99d89b447efd8a0a2e8b2be76e::coin::T
    CoinId::from_addresses_str(
        "0xae478ff7d83ed072dbc5e264250e67ef58f57c99d89b447efd8a0a2e8b2be76e",
        "coin",
        "T",
    )
}

pub fn whusdc_coin_id() -> CoinId {
    // "0xa2eda21a58856fda86451436513b867c97eecb4ba099da5775520e0f7492e852::coin::T"
    CoinId::from_addresses_str(
        "0xa2eda21a58856fda86451436513b867c97eecb4ba099da5775520e0f7492e852",
        "coin",
        "T",
    )
}

pub fn whweth_coin_id() -> CoinId {
    // "0xcc8a89c8dce9693d354449f1f73e60e14e347417854f029db5bc8e7454008abb::coin::T"
    CoinId::from_addresses_str(
        "0xcc8a89c8dce9693d354449f1f73e60e14e347417854f029db5bc8e7454008abb",
        "coin",
        "T",
    )
}

pub fn aptoge_coin_id() -> CoinId {
    // "0x5c738a5dfa343bee927c39ebe85b0ceb95fdb5ee5b323c95559614f5a77c47cf::coin::Aptoge"
    CoinId::from_addresses_str(
        "0x5c738a5dfa343bee927c39ebe85b0ceb95fdb5ee5b323c95559614f5a77c47cf",
        "coin",
        "Aptoge",
    )
}

pub fn zusdc_coin_id() -> CoinId {
    CoinId::from_addresses_str(
        "0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa",
        "asset",
        "USDC",
    )
}

pub fn zusdt_coin_id() -> CoinId {
    CoinId::from_addresses_str(
        "0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa",
        "asset",
        "USDT",
    )
}

pub fn zweth_coin_id() -> CoinId {
    //coin_id = "0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa::asset::WETH"
    CoinId::from_addresses_str(
        "0xf22bede237a07e121b56d91a491eb7bcdfd1f5907926a9e58338f964a01b17fa",
        "asset",
        "WETH",
    )
}

pub fn thala_coin_id() -> CoinId {
    CoinId::from_addresses_str(
        "0x7fd500c11216f0fe3095d0c4b8aa4d64a4e2e04f83758462f2b127255643615",
        "thl_coin",
        "THL",
    )
}

pub fn thala_mod_coin_id() -> CoinId {
    CoinId::from_addresses_str(
        "0x6f986d146e4a90b828d8c12c14b6f4e003fdff11a8eecceceb63744363eaac01",
        "mod_coin",
        "MOD",
    )
}

pub fn thala_thapt_coin_id() -> CoinId {
    CoinId::from_addresses_str(
        "0xfaf4e633ae9eb31366c9ca24214231760926576c7b625313b3688b5e900731f6",
        "staking",
        "ThalaAPT",
    )
}

pub fn thala_amapt_coin_id() -> CoinId {
    CoinId::from_addresses_str(
        "0x111ae3e5bc816a5e63c2da97d0aa3886519e0cd5e4b046659fa35796bd11542a",
        "amapt_token",
        "AmnisApt",
    )
}

pub fn tortuga_staked_aptos_coin_id() -> CoinId {
    CoinId::from_addresses_str(
        "0x84d7aeef42d38a5ffc3ccef853e1b82e4958659d16a7de736a29c55fbbeb0114",
        "staked_aptos_coin",
        "StakedAptosCoin",
    )
}

pub fn apt_coin_id() -> CoinId {
    CoinId::from_addresses_str("0x1", "aptos_coin", "AptosCoin")
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Coin {
    pub name: String,
    /// coin_id e.g.  "x00000000::module_name::struct_name"
    pub coin_id: CoinId,

    #[serde(default)]
    pub decimal: CoinDecimal,

    #[serde(default)]
    pub stable_coin: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub struct CoinDecimal(u8);

impl Deref for CoinDecimal {
    type Target = u8;
    fn deref(&self) -> &u8 {
        &self.0
    }
}

impl Default for CoinDecimal {
    fn default() -> Self {
        Self(8u8)
    }
}
impl From<u8> for CoinDecimal {
    fn from(v: u8) -> Self {
        Self(v)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoinId(String);
impl CoinId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn from_type_tag(s: &str) -> Self {
        Self(s.to_string())
    }

    pub fn from_addresses_str(address: &str, module_name: &str, struct_name: &str) -> Self {
        let coin_type = format!("{}::{}::{}", address, module_name, struct_name);
        Self(coin_type)
    }

    pub fn from_addresses(
        address: &Address,
        module_name: &IdentifierWrapper,
        struct_name: &IdentifierWrapper,
    ) -> Self {
        let coin_type = format!("{}::{}::{}", address, module_name, struct_name);
        Self(coin_type)
    }

    pub fn as_address_and_identifiers(&self) -> Result<(AccountAddress, Identifier, Identifier)> {
        let values = self.0.split("::").collect::<Vec<&str>>();
        let address = values.get(0).ok_or(anyhow!("invalid coin id {self:?}"))?;
        let address = hex_to_account_address(address)?;

        let module = values.get(1).ok_or(anyhow!("invalid coin id {self:?}"))?;
        let module = Identifier::new(module.to_string())?;

        let name = values.get(2).ok_or(anyhow!("invalid coin id {self:?}"))?;
        let name = Identifier::new(name.to_string())?;

        Ok((address, module, name))
    }
}

impl Coin {
    pub fn new<S: Into<String>>(
        name: S,
        coin_id: CoinId,
        decimal: Option<CoinDecimal>,
        stable_coin: bool,
    ) -> Self {
        Coin {
            name: name.into(),
            coin_id,
            decimal: decimal.unwrap_or_default(),
            stable_coin,
        }
    }

    pub fn is_apt_coin(&self) -> bool {
        self.coin_id == apt_coin_id()
    }

    pub fn is_stable(&self) -> bool {
        self.stable_coin
    }

    pub fn as_coin_id(&self) -> &CoinId {
        &self.coin_id
    }

    pub fn from_tag(struct_tag: &MoveStructTag) -> Result<Self> {
        let coin_id =
            CoinId::from_addresses(&struct_tag.address, &struct_tag.module, &struct_tag.name);
        let default_coin_name = struct_tag.name.to_string();

        let registered_coins = REGISTERED_COINS.read();
        let well_known_coin = registered_coins.get_by_coin_id(&coin_id);
        let (coin_name, decimal, stable) = match well_known_coin {
            Some(well_known_coin) => (
                well_known_coin.name.to_string(),
                well_known_coin.decimal,
                well_known_coin.stable_coin,
            ),
            None => (default_coin_name, CoinDecimal::default(), false),
        };

        Ok(Coin::new(coin_name, coin_id, Some(decimal), stable))
    }

    pub fn from_str(type_str: &str) -> Result<Self> {
        let tag = MoveStructTag::from_str(type_str)?;
        Self::from_tag(&tag)
    }

    pub fn from_hex_strs(
        address: &str,
        hex_module_name: &str,
        hex_struct_name: &str,
    ) -> Result<Self> {
        let module_name = String::from_utf8(hex::decode(&hex_module_name[2..])?)?;
        let struct_name = String::from_utf8(hex::decode(&hex_struct_name[2..])?)?;

        let coin_id =
            CoinId::from_addresses_str(address, module_name.as_str(), struct_name.as_str());
        let default_coin_name = struct_name;
        let registered_coins = REGISTERED_COINS.read();
        let well_known_coins = registered_coins.get_by_coin_id(&coin_id);

        let (coin_name, decimal, stable) = match well_known_coins {
            Some(well_known_coin) => (
                well_known_coin.name.to_string(),
                well_known_coin.decimal,
                well_known_coin.stable_coin,
            ),
            None => (default_coin_name, CoinDecimal::default(), false),
        };

        Ok(Coin::new(coin_name, coin_id, Some(decimal), stable))
    }
}

#[macro_export]
macro_rules! move_type_to_coin_by_idx {
    ($move_types:expr,$idx:expr) => {
        match $crate::resource::move_types_vec_to_struct!($move_types, $idx) {
            Ok(coin_struct) => Coin::from_tag(&coin_struct),
            Err(e) => Err(e),
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinAmount {
    pub coin: Coin,
    pub amount: u64,
    pub amount_integer: u64,
    pub amount_fraction: u64,
}

impl CoinAmount {
    pub fn decimal_amount_str(coin_amount: &CoinAmount) -> String {
        format!(
            "{}.{}",
            coin_amount.amount_integer, coin_amount.amount_fraction
        )
    }
    pub fn is_apt_coin(&self) -> bool {
        self.coin.is_apt_coin()
    }

    pub fn is_zero(&self) -> bool {
        self.amount == 0
    }

    fn from_resource(resource: &MoveResource) -> Result<u64> {
        if let Some(serde_json::Value::Object(obj)) = resource.data.0.get(&COIN_AMOUNT_IDENTIFIER) {
            if let Some(value) = obj.get("value") {
                if let Some(amount_str) = value.as_str() {
                    if let Ok(amount) = amount_str.parse::<u64>() {
                        return Ok(amount);
                    }
                }
            }
        }
        Err(anyhow!("could not get coin amount {resource:?}"))
    }

    pub fn from_resources(resources: &Vec<MoveResource>, ignore_zero: bool) -> Vec<CoinAmount> {
        resources
            .iter()
            .filter_map(|resource| match CoinAmount::try_from(resource) {
                Ok(coin_amount) => {
                    if ignore_zero && coin_amount.amount == 0 {
                        None
                    } else {
                        Some(coin_amount)
                    }
                },
                Err(_e) => None,
            })
            .collect()
    }

    pub fn load_resource(
        address_hex: &str,
        ignore_zero: bool,
        db_reader: Arc<dyn DbReader>,
        target_version: Option<u64>,
        move_converter: &MoveConverter<DbStateView>,
    ) -> Result<Vec<Self>> {
        let resources = read_db::get_resources_of_account(
            db_reader.clone(),
            address_hex,
            target_version,
            move_converter,
        )?;
        Ok(Self::from_resources(&resources.resources, ignore_zero))
    }
}

impl TryFrom<&MoveResource> for CoinAmount {
    type Error = AnyhowError;

    fn try_from(resource: &MoveResource) -> Result<Self, Self::Error> {
        if struct_tag_from_resource_without_generics(&resource.typ) == COIN_RESOURCE_TAG {
            let coin_tag = if let Some(MoveType::Struct(struct_tag)) =
                resource.typ.generic_type_params.get(0)
            {
                struct_tag
            } else {
                return Err(anyhow!(
                    "could not get coin generics tag {:?}",
                    resource.typ
                ));
            };

            let coin = Coin::from_tag(coin_tag)?;
            let amount = CoinAmount::from_resource(resource)?;

            let (q, r) = amount.div_rem(&10u64.pow(*coin.decimal as u32));

            Ok(CoinAmount {
                coin,
                amount,
                amount_integer: q,
                amount_fraction: r,
            })
        } else {
            Err(anyhow!("not coin resource"))
        }
    }
}

#[derive(Default)]
pub struct CoinPool(HashMap<CoinId, Coin>);

#[derive(Deserialize)]
struct CoinPoolToml {
    coin: Vec<Coin>,
}

impl CoinPool {
    fn get_by_coin_id(&self, coin_id: &CoinId) -> Option<&Coin> {
        self.0.get(coin_id)
    }

    pub fn load() -> Result<Self> {
        let account_resource = crate::path::PROJECT_DIR
            .get_file("resources/coin.toml")
            .unwrap();
        let coins: CoinPoolToml = toml::from_slice(account_resource.contents())?;

        Ok(Self(
            coins
                .coin
                .into_iter()
                .map(|each_coin| (each_coin.as_coin_id().clone(), each_coin))
                .collect(),
        ))
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Coin> {
        self.0.values().find(|v| v.name == name)
    }

    pub fn coin_ids(&self) -> Vec<CoinId> {
        self.0
            .values()
            .into_iter()
            .map(|coin| coin.as_coin_id().clone())
            .collect()
    }
}
