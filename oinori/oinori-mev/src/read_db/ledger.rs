use anyhow::Result;

use aptos_types::ledger_info::{LedgerInfoWithSignatures, LedgerInfoWithV0};

use aptos_storage_interface::DbReader;
use std::sync::Arc;

pub fn get_latest_ledger(db_reader: Arc<dyn DbReader>) -> Result<LedgerInfoWithV0> {
    let latest_ledger_info = db_reader.get_latest_ledger_info()?;
    let LedgerInfoWithSignatures::V0(latest_ledger_info) = latest_ledger_info;
    Ok(latest_ledger_info)
}
