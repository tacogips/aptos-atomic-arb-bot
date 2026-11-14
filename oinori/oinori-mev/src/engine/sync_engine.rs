use crate::envs::*;
use crate::ledger::get_ledger_info_via_rpc;
use crate::metrics::*;

use anyhow::Result;

// not need to catch the exact latest value if the tps's too high
const SYNC_VERSION_BUFFER_RANGE: u64 = 50_000;

pub enum SyncState {
    SyncToLatest(u64),
    Proceeding((u64, u64)),
}

pub async fn sync_status(env: &OinoriEnvs) -> Result<SyncState> {
    let latest_remote_status = get_ledger_info_via_rpc(&env.rpc_url).await?;
    let latest_metrics = get_metrics(&env.self_metrics_url).await?;

    if latest_metrics.synced_version + SYNC_VERSION_BUFFER_RANGE >= latest_remote_status.version {
        Ok(SyncState::SyncToLatest(latest_metrics.synced_version))
    } else {
        Ok(SyncState::Proceeding((
            latest_metrics.synced_version,
            latest_remote_status.version,
        )))
    }
}
