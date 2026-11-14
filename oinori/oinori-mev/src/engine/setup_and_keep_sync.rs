use crate::account::get_account_via_rpc;
use crate::coins::CoinAmount;
use crate::engine::get_last_user_tx_got_at_nano_sec;
use crate::engine::set_already_meved_version;
use crate::envs::*;
use crate::ledger::get_ledger_info_via_rpc;
use crate::swap::{SwapContract, SwapRouterPool, SwapRouterPoolOptions};
use anyhow::Result;
use aptos_api_types::{AsConverter, MoveConverter};
use aptos_logger::prelude::*;

use aptos_storage_interface::{
    state_view::{DbStateView, LatestDbStateCheckpointView},
    DbReader,
};

use super::sync_engine;
use super::Engine;
use aptos_vm::data_cache::AsMoveResolver;
use chrono::FixedOffset;
use chrono::Utc;

use parking_lot::RwLock;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

const KEEP_SYNC_DURATION_SEC: u64 = 40;

static ENGINE_SYNCED_VERSION: AtomicU64 = AtomicU64::new(0);

pub fn get_boostrap_synced() -> bool {
    get_engine_synced_version() != 0
}

pub fn get_engine_synced_version() -> u64 {
    ENGINE_SYNCED_VERSION.load(std::sync::atomic::Ordering::Acquire)
}

pub fn set_engine_synced_version(version: u64) {
    ENGINE_SYNCED_VERSION.store(version, std::sync::atomic::Ordering::Release);
}

static ENGINE_LAST_SYNC_AT: AtomicU64 = AtomicU64::new(0);
pub fn get_engine_last_sync_at() -> u64 {
    ENGINE_LAST_SYNC_AT.load(std::sync::atomic::Ordering::Acquire)
}

pub fn set_engine_last_sync_at_synced() {
    let now = Utc::now().timestamp() as u64;
    ENGINE_LAST_SYNC_AT.store(now, std::sync::atomic::Ordering::Release);
}

static ENGINE_LAST_START: AtomicU64 = AtomicU64::new(0);
pub fn get_engine_last_start_at() -> u64 {
    ENGINE_LAST_START.load(std::sync::atomic::Ordering::Acquire)
}

pub fn set_engine_last_start_at_synced() {
    let now = Utc::now().timestamp() as u64;
    ENGINE_LAST_START.store(now, std::sync::atomic::Ordering::Release);
}

pub async fn setup_and_keep_sync_engine(
    envs: OinoriEnvs,
    mev_engine: Arc<RwLock<Engine>>,
    swap_router_pool: Arc<RwLock<SwapRouterPool>>,
    db_reader: Arc<dyn DbReader>,
) {
    let node_id = envs.node_id.clone();

    let db_reader_for_setup = db_reader.clone();

    let node_id_for_update = envs.node_id.clone();

    let mev_engine_for_update = mev_engine.clone();

    let envs_for_update = envs.clone();
    let db_reader_for_update_state = db_reader.clone();

    let swap_router_pool_for_update = swap_router_pool.clone();

    let jh = tokio::task::spawn(async move {
        start_up_engine(
            node_id,
            envs,
            mev_engine,
            swap_router_pool.clone(),
            db_reader_for_setup,
        )
        .await
    });

    // keep syncing infos
    let keep_syncing_jh = tokio::task::spawn(async move {
        keep_sync_data_diffs(
            node_id_for_update,
            envs_for_update,
            mev_engine_for_update,
            swap_router_pool_for_update,
            db_reader_for_update_state,
        )
        .await
    });

    let _ = jh.await;
    let _ = keep_syncing_jh.await;
}

async fn keep_sync_data_diffs(
    node_id: String,
    envs: OinoriEnvs,
    mev_engine: Arc<RwLock<Engine>>,
    swap_router_pool: Arc<RwLock<SwapRouterPool>>,
    db_reader: Arc<dyn DbReader>,
) {
    let mut update_timer =
        tokio::time::interval(tokio::time::Duration::from_secs(KEEP_SYNC_DURATION_SEC));
    loop {
        update_timer.tick().await;
        if get_engine_synced_version() == 0 {
            // Setup not finished yet
            continue;
        }

        info!("start keep sync load_pool_from_db()");

        // Check for stalling
        {
            let last_user_tx_at_nano_sec = get_last_user_tx_got_at_nano_sec();
            if last_user_tx_at_nano_sec != 0 {
                let now_nano_sec = Utc::now().timestamp_nanos() as u64;
                if now_nano_sec > last_user_tx_at_nano_sec {
                    // 5 minute stall detection
                    if now_nano_sec - last_user_tx_at_nano_sec > 1_000_000_000 * 60 * 5 {
                        error!("mev stall after 5 min.panicing");
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        panic!("stall!!!");
                    }
                }
            }
        }

        // Sync sequence number
        let account_address = mev_engine
            .read()
            .transaction_sender_address_and_key
            .as_account_address()
            .unwrap();

        match get_account_via_rpc(account_address, &envs.rpc_url).await {
            Ok(latest_account) => {
                let current_seq_no = mev_engine.read().sender_seq_no();
                if latest_account.sequence_number != current_seq_no.inner() {
                    let latest_sequence_number = {
                        let mev_engine_writer = mev_engine.try_write();
                        let latest_sequence_number = match mev_engine_writer {
                            Some(mut mev_engine_writer) => {
                                let last_mev_fired_at =
                                    crate::engine::mev_runner::get_last_mev_fired_at_nano_sec();
                                let now_nano_sec = Utc::now().timestamp_nanos() as u64;
                                if now_nano_sec > last_mev_fired_at
                                    && (now_nano_sec - last_mev_fired_at) > 30_000_000_000
                                // after 30 sec mev fired
                                {
                                    mev_engine_writer
                                        .init_sender_with_seq_no(latest_account.sequence_number);

                                    latest_account.sequence_number
                                } else {
                                    0u64
                                }
                            },
                            None => {
                                drop(mev_engine_writer);
                                warn!("could not sync seq no. could not get mev engine lock");
                                continue;
                            },
                        };
                        latest_sequence_number
                    };

                    if latest_sequence_number != 0 {
                        info!("update seq no with {}", latest_sequence_number);
                    }
                }
            },
            Err(e) => {
                warn!("periodically update seq no error {:?}", e);
            },
        }

        // update account balance
        match get_ledger_info_via_rpc(&envs.rpc_url).await {
            Ok(latest_remote_status) => match db_reader.clone().latest_state_checkpoint_view() {
                Err(e) => {
                    warn!("failed to sync account balance. error occured during getting latest db state:{e:?}");
                },
                Ok(state_view) => {
                    let account_resource = {
                        let move_converter = state_view.as_converter(db_reader.clone(), None);

                        CoinAmount::load_resource(
                            &account_address.to_hex_literal(),
                            false,
                            db_reader.clone(),
                            Some(latest_remote_status.version),
                            &move_converter,
                        )
                    };

                    match account_resource {
                        Ok(resources) => {
                            let apt_coin = resources.iter().find(|coin| coin.is_apt_coin());
                            match apt_coin {
                                Some(latest_remove_coin_amount) => {
                                    if mev_engine.read().sender_apt_balance.unwrap_or(0)
                                        != latest_remove_coin_amount.amount
                                    {
                                        let latest_coin_amount = {
                                            let mev_engine_writer = mev_engine.try_write();

                                            let latest_coin_amount = match mev_engine_writer {
                                                Some(mut mev_engine_writer) => {
                                                    mev_engine_writer.sender_apt_balance =
                                                        Some(latest_remove_coin_amount.amount);
                                                    latest_remove_coin_amount.amount
                                                },
                                                None => {
                                                    drop(mev_engine_writer);
                                                    warn!("could not sync account resource. could not get mev engine lock");
                                                    continue;
                                                },
                                            };
                                            latest_coin_amount
                                        };

                                        info!("update account balance {}", latest_coin_amount);
                                    }
                                },
                                None => {
                                    warn!("periodically update account balance error. apt coin not found");
                                },
                            }
                        },
                        Err(e) => {
                            warn!("periodically update account balance error. failed to get account resource {:?}", e);
                        },
                    }
                },
            },
            Err(e) => {
                warn!("periodically update account balance error {:?}", e);
            },
        }
    }
    println!("keep syncing exit somehow ");
    error!("keep syncing exit somehow");
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    panic!("panic to restart the whole process");
}

async fn start_up_engine(
    node_id: String,
    envs: OinoriEnvs,
    mev_engine: Arc<RwLock<Engine>>,
    swap_router_pool: Arc<RwLock<SwapRouterPool>>,
    db_reader_for_setup: Arc<dyn DbReader>,
) {
    let mut ineterval = tokio::time::interval(tokio::time::Duration::from_secs(1));

    info!("========== OINORI ENGINE STARTUP ==========");
    info!("[OINORI] Waiting for blockchain sync...");
    info!("[OINORI] Engine startup initiated");

    set_engine_last_start_at_synced();
    loop {
        ineterval.tick().await;

        info!("[OINORI] Checking blockchain sync status...");
        match sync_engine::sync_status(&envs).await {
            Err(e) => {
                error!("[OINORI] Sync engine error: {e:?}");
            },
            Ok(sync_status) => match sync_status {
                sync_engine::SyncState::SyncToLatest(latest_synced_version) => {
                    info!("[OINORI] *** SYNC COMPLETE *** version: {}", latest_synced_version);

                    let account_address = mev_engine
                        .read()
                        .transaction_sender_address_and_key
                        .as_account_address()
                        .unwrap();

                    let latest_account = match get_account_via_rpc(account_address, &envs.rpc_url)
                        .await
                    {
                        Ok(latest_account) => latest_account,
                        Err(e) => {
                            warn!("get account error {e:?}");
                            continue;
                        },
                    };

                    let sender_seq_no = {
                        let mev_engine_writer = mev_engine.try_write();
                        let mut mev_engine_writer = match mev_engine_writer {
                            Some(mev_engine_writer) => mev_engine_writer,
                            None => {
                                warn!("could not sync to latest. could not get mev engine lock");
                                continue;
                            },
                        };

                        mev_engine_writer.init_sender_with_seq_no(latest_account.sequence_number);
                        set_engine_synced_version(latest_synced_version);
                        set_already_meved_version(latest_synced_version);

                        mev_engine_writer.sender_seq_no().inner()
                    };

                    info!(
                        "!!! set latest synced version {},  {} sequence_no:{}",
                        latest_synced_version,
                        get_engine_synced_version(),
                        sender_seq_no,
                    );

                    let result = {
                        let latests_swap_router_pool = load_pool_from_db(
                            Some(latest_synced_version),
                            db_reader_for_setup.clone(),
                        );
                        let swap_update_result = match latests_swap_router_pool {
                            Ok((_, latests_swap_router_pool)) => {
                                swap_router_pool
                                    .write()
                                    .overwrite_with(latests_swap_router_pool);
                                Ok(swap_router_pool.read().swaps_set())
                            },
                            Err(e) => Err(e),
                        };
                        swap_update_result
                    };
                    match result {
                        Ok(available_swaps) => {
                            let available_swaps_len = {
                                let mev_engine_writer = mev_engine.try_write();
                                let mut mev_engine_writer = match mev_engine_writer {
                                    Some(mev_engine_writer) => mev_engine_writer,
                                    None => {
                                        warn!("could not sync to latest. could not get mev engine lock");
                                        continue;
                                    },
                                };

                                mev_engine_writer.update_available_swaps(available_swaps);
                                mev_engine_writer.available_swaps.len()
                            };
                            info!(
                                "update swap router pool with {} swaps",
                                available_swaps_len
                            );
                            return;
                        },
                        Err(e) => {
                            warn!("get latest pool error {e:?}");
                        },
                    };
                },
                sync_engine::SyncState::Proceeding((latest_synced_version, remote_vesion)) => {
                    let diff = if remote_vesion > latest_synced_version {
                        remote_vesion - latest_synced_version
                    } else {
                        0
                    };
                    let progress_pct = if remote_vesion > 0 {
                        (latest_synced_version as f64 / remote_vesion as f64 * 100.0) as u32
                    } else {
                        0
                    };

                    info!(
                        "[OINORI] SYNCING: local={}, remote={}, behind={} blocks ({}%)",
                        latest_synced_version, remote_vesion, diff, progress_pct
                    );
                },
            },
        }
    }
}

fn load_pool_from_db(
    version: Option<u64>,
    db_reader: Arc<dyn DbReader>,
) -> Result<(u64, SwapRouterPool)> {
    let wellknown_coin_ids = crate::coins::REGISTERED_COINS.read().coin_ids();

    let state_view = db_reader.clone().latest_state_checkpoint_view().unwrap();

    let move_converter = state_view.as_converter(db_reader.clone(), None);

    let version = match version {
        Some(v) => v,
        None => db_reader.get_latest_ledger_info_version()?,
    };

    let swap_router_pool = SwapRouterPool::load_from_db(
        db_reader.clone(),
        Some(version),
        &move_converter,
        Some(SwapRouterPoolOptions {
            exclude_swaps: vec![SwapContract::Cetus],
            coin_filter: wellknown_coin_ids,
        }),
    )?;

    Ok((version, swap_router_pool))
}
