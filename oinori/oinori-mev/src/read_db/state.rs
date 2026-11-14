use super::ledger;
use crate::account_address::hex_to_account_address;
use anyhow::{anyhow, Result};
use aptos_types::{
    access_path::{AccessPath, Path},
    state_store::{
        state_key::{inner::StateKeyInner, prefix::StateKeyPrefix, StateKey},
        state_value::StateValue,
    },
};
use move_core_types::language_storage::StructTag;
use std::sync::Arc;

use aptos_api_types::MoveResource;

use aptos_storage_interface::state_view::DbStateView;

use aptos_api_types::MoveConverter;
use aptos_storage_interface::DbReader;
use bytes::Bytes;

struct ResourceData {
    struct_tag: StructTag,
    data: Bytes,
}

fn translate_state(state_key: StateKey, state_value: StateValue) -> Option<Result<ResourceData>> {
    match state_key.inner() {
        StateKeyInner::AccessPath(AccessPath { address: _, path }) => {
            match Path::try_from(path.as_slice()) {
                Ok(Path::Resource(struct_tag)) => Some(Ok(ResourceData {
                    struct_tag,
                    data: state_value.bytes().clone(),
                })),
                Ok(Path::ResourceGroup(struct_tag)) => Some(Ok(ResourceData {
                    struct_tag,
                    data: state_value.bytes().clone(),
                })),
                Ok(Path::Code(_)) => None,
                Err(e) => Some(Err(anyhow::Error::from(e))),
            }
        },
        other => Some(Err(anyhow!("invalid resource key {other:?}"))),
    }

}
pub struct MoveResourceAndVersion {
    pub version: u64,
    pub resources: Vec<MoveResource>,
}

pub fn get_resources_of_account(
    db_reader: Arc<dyn DbReader>,
    account_address_hex: &str,
    target_version: Option<u64>,
    move_converter: &MoveConverter<DbStateView>,
) -> Result<MoveResourceAndVersion> {
    let target_address = hex_to_account_address(account_address_hex)?;
    let target_version = target_version.map(Ok).unwrap_or_else(|| {
        ledger::get_latest_ledger(db_reader.clone())
            .map(|ledger_info| ledger_info.ledger_info().version())
    })?;

    let resources = db_reader
        .get_prefixed_state_value_iterator(
            &StateKeyPrefix::from(target_address),
            None,
            target_version,
        )?
        .filter_map(|res| match res {
            Ok((state_key, state_value)) => translate_state(state_key, state_value),
            Err(e) => Some(Err(e.into())),
        })
        .collect::<Result<Vec<ResourceData>>>()?;

    let resources = resources
        .iter()
        .map(|resource| move_converter.try_into_resource(&resource.struct_tag, &resource.data))
        .collect::<Result<Vec<MoveResource>>>()?;

    Ok(MoveResourceAndVersion {
        version: target_version,
        resources,
    })
}
