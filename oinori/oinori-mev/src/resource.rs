use anyhow::{anyhow, Result};
use aptos_api_types::{IdentifierWrapper, MoveStructTag, MoveStructValue, MoveType};

use crate::coins::Coin;

use move_core_types::identifier::Identifier;

use aptos_storage_interface::{
    state_view::{DbStateView, LatestDbStateCheckpointView},
    DbReader,
};
use aptos_vm::data_cache::AsMoveResolver;
use std::sync::Arc;

pub fn struct_from_generics<'a>(
    generics: &'a Vec<MoveType>,
    idx: usize,
) -> Result<&'a MoveStructTag> {
    if let Some(MoveType::Struct(move_struct)) = generics.get(idx) {
        Ok(move_struct)
    } else {
        Err(anyhow!("no generics :{:?} idx:{}", generics, idx))
    }
}

pub fn coin_from_generics(generics: &Vec<MoveType>, idx: usize) -> Result<Coin> {
    let move_struct = struct_from_generics(generics, idx)?;

    Coin::from_tag(move_struct)
}

pub fn struct_tag_from_resource_without_generics(tag: &MoveStructTag) -> String {
    format!("{}::{}::{}", tag.address, tag.module, tag.name)
}

pub fn get_value_from_struct_value<'a>(
    key: &str,
    struct_value: &'a MoveStructValue,
) -> Result<Option<&'a serde_json::Value>> {
    let key = new_identifyer_wrapper(key)?;
    Ok(struct_value.0.get(&key))
}

pub fn new_identifyer_wrapper(s: &str) -> Result<IdentifierWrapper> {
    Ok(IdentifierWrapper(Identifier::new(s)?))
}

#[macro_export]
macro_rules! move_types_vec_to_struct {
    ($move_types:expr,$idx:expr) => {
        if let Some(move_type) = $move_types.get($idx) {
            if let aptos_api_types::MoveType::Struct(coin_struct) = move_type {
                Ok(coin_struct)
            } else {
                Err(anyhow::anyhow!(
                    "specified move type is not a struct {:?} idx:{:?}",
                    $move_types,
                    $idx
                ))
            }
        } else {
            Err(anyhow::anyhow!(
                "specified move type is not a struct {:?} idx:{:?}",
                $move_types,
                $idx
            ))
        }
    };
}
pub use move_types_vec_to_struct;

#[macro_export]
macro_rules! get_value_from_json_obj {
    ($json_obj:expr,$key:expr) => {
        if let serde_json::Value::Object(obj) = $json_obj {
            match obj.get($key) {
                Some(value) => Ok(value),
                None => Err(anyhow!(
                    "field not found in json obj {:?} {:?}",
                    $json_obj,
                    $key
                )),
            }
        } else {
            Err(anyhow!("invalid liquid reserve object {:?}", $json_obj))
        }
    };
}
pub use get_value_from_json_obj;

#[macro_export]
macro_rules! get_value_from_struct_with_type {
    ($data:expr, $key:expr, $typ:ty) => {
        match get_value_from_struct_value($key, $data) {
            Ok(v) => {
                if let Some(v) = v {
                    serde_json::from_value::<$typ>(v.clone())
                        .map_err(|e| anyhow::anyhow!("{:?}", e))
                } else {
                    Err(anyhow::anyhow!("no {} in resource", $key))
                }
            },
            Err(e) => Err(e),
        }
    };
}
pub(crate) use get_value_from_struct_with_type;
