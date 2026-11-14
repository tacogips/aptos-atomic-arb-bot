use super::ledger::get_latest_ledger;
use crate::account_address::hex_to_hash_value;
use anyhow::Result;

use aptos_storage_interface::DbReader;
use std::sync::Arc;

use aptos_types::transaction::{TransactionOutputListWithProof, TransactionWithProof};

pub fn get_tx_by_hash(
    tx_hash_hex: &str,
    db_reader: Arc<dyn DbReader>,
    latest_version: Option<u64>,
) -> Result<Option<TransactionWithProof>> {
    let tx_hash = hex_to_hash_value(tx_hash_hex)?;

    let latest_version = given_latest_version_or_fetch(latest_version, db_reader.clone())?;
    Ok(db_reader.get_transaction_by_hash(tx_hash, latest_version, true)?)
}

pub fn get_tx_by_version(
    version: u64,
    db_reader: Arc<dyn DbReader>,
    latest_version: Option<u64>,
) -> Result<TransactionWithProof> {
    let latest_version = given_latest_version_or_fetch(latest_version, db_reader.clone())?;
    Ok(db_reader.get_transaction_by_version(version, latest_version, true)?)
}

pub const MAX_TX_OUTPUT_LIMIT: u64 = 10000;

pub fn get_tx_outputs(
    start_version: u64,
    limit: u64,
    db_reader: Arc<dyn DbReader>,
    latest_version: Option<u64>,
) -> Result<TransactionOutputListWithProof> {
    let latest_version = given_latest_version_or_fetch(latest_version, db_reader.clone())?;

    Ok(db_reader.get_transaction_outputs(start_version, limit, latest_version)?)
}

fn given_latest_version_or_fetch(
    latest_version: Option<u64>,
    db_reader: Arc<dyn DbReader>,
) -> Result<u64> {
    match latest_version {
        Some(latest_version) => Ok(latest_version),
        None => {
            let latest_ledger_info = get_latest_ledger(db_reader.clone())?;
            Ok(latest_ledger_info.ledger_info().version())
        },
    }
}
