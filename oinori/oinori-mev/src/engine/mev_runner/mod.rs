use aptos_logger::prelude::*;
use crate::coins::*;
use crate::engine::{call_order_contract, call_order_contract::datetime_to_jst, find_route::*, route_filter, Engine};
use crate::swap::{SwapContract, SwapResource, SwapRouterPool};
use crate::transaction::builder::*;
use anyhow::Result;
use aptos_data_client::client::AptosDataClient;
use aptos_types::account_address::AccountAddress;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

const MEV_FIRE_THRESHOLD_PROFIT_APT: f64 = 0.001;

// never plunge under this
const GUARD_UNDER_LIMIT_BALANCE: u64 = 90_0000_0000;
const USE_LOAN_THREASHOLD_AMOUNT: f64 = 90.0;

const MEV_FIRE_THRESHOLD_PROFIT_APT_U64: Lazy<u64> =
    Lazy::new(|| apt_amount(MEV_FIRE_THRESHOLD_PROFIT_APT));

const USE_LOAN_THREASHOLD_AMOUNT_U64: Lazy<u64> =
    Lazy::new(|| apt_amount(USE_LOAN_THREASHOLD_AMOUNT));

const ENSURE_MIN_OUT: bool = true;

static LAST_MEV_FIRED_AT: AtomicU64 = AtomicU64::new(0);

pub fn get_last_mev_fired_at_nano_sec() -> u64 {
    LAST_MEV_FIRED_AT.load(std::sync::atomic::Ordering::Acquire)
}

fn set_last_mev_fired_nano_sec(at_nano_sec: u64) {
    LAST_MEV_FIRED_AT.store(at_nano_sec, std::sync::atomic::Ordering::Release);
}

pub struct AccountInfoForMev {
    pub sender_account_address: AccountAddress,
    pub seq_no_at_contract: AccountSeqNo,
    pub account_apt_balance: Option<u64>,
}

pub async fn run_mev(
    node_id: &str,
    account_info: AccountInfoForMev,

    version: u64,
    tx_sender_address: AccountAddress,
    swap_resources: Vec<SwapResource>,
    mev_engine: Arc<RwLock<Engine>>,
    swap_router_pool: Arc<RwLock<SwapRouterPool>>,

    aptos_net_data_client: &AptosDataClient,

    mev_receipt_tx_at: DateTime<Utc>,
    dry_run: bool,
) -> Result<()> {
    let affected_swap_updates = swap_router_pool.write().update_pool_with_resources(
        version,
        swap_resources.clone(),
        false,
    )?;

    if affected_swap_updates.is_empty() {
        warn!(
            "new swap resource changes didn't any affect to swap pool {},{:?}",
            version, swap_resources
        );

        return Ok(());
    }

    if account_info.sender_account_address == tx_sender_address {
        warn!("skip self tx version:{} {}", version, tx_sender_address);
        return Ok(());
    }

    let mev_start_at = Utc::now();

    let duration_usec_tx_receipt_and_mev_start = (mev_start_at - mev_receipt_tx_at)
        .num_microseconds()
        .unwrap() as u64;

    let route_filter::SearchRouteCondition {
        swap_filters_for_steps,
        coin_filters_for_steps,
        input_amounts,
    } = route_filter::route_filter_by_affected_resource(&affected_swap_updates);

    //---  contain thala or not -------------------------------
    let exclude_in_all_route_swap =
        if route_filter::thala_resource_might_affected(&affected_swap_updates) {
            // contain thala in search route
            None
        } else {
            // exclude thala
            Some(SwapContract::Thala)
        };

    let cloned_swap_router_pool = swap_router_pool.read().clone();
    match search_route(
        &node_id,
        version,
        affected_swap_updates.clone(),
        &cloned_swap_router_pool,
        ProfitAmout(*MEV_FIRE_THRESHOLD_PROFIT_APT_U64),
        UseLoanAmount(*USE_LOAN_THREASHOLD_AMOUNT_U64),
        ENSURE_MIN_OUT,
        input_amounts.clone(),
        swap_filters_for_steps.clone(),
        coin_filters_for_steps.clone(),
        exclude_in_all_route_swap.clone(),
    )
    .await
    {
        Err(e) => {
            println!(
                "=== finish search route with error === {} {:?}",
                datetime_to_jst(Utc::now()),
                e
            );
            error!("search route error {:?} version:{}", e, version);
            Err(e)
        },
        Ok(None) => Ok(()),
        Ok(Some((search_route_result, args))) => {
            let start_fire_contact = Utc::now();

            println!(
                "=== start fire contract === {}",
                datetime_to_jst(Utc::now())
            );
            let FireFoundRouteContractResult {
                skip_cause_too_soon,
                commited_tx_hash,
                commit_tx_msg,
            } = fire_found_route_contract(
                &search_route_result,
                args,
                node_id,
                &account_info,
                version,
                &swap_resources,
                mev_engine,
                &cloned_swap_router_pool,
                aptos_net_data_client,
                dry_run,
            )
            .await?;

            println!(
                "=== finish fire contract === {}",
                datetime_to_jst(Utc::now())
            );

            let fire_contract_duration = Utc::now() - start_fire_contact;
            let mev_take_duration = Utc::now() - mev_start_at;

            info!(
                "MEV fired: version={}, account={}, seq_no={}, skip_too_soon={}, tx_hash={:?}, msg={:?}, mev_duration_us={}, fire_duration_us={}",
                version,
                account_info.sender_account_address.to_hex_literal(),
                *account_info.seq_no_at_contract,
                skip_cause_too_soon,
                commited_tx_hash,
                commit_tx_msg,
                mev_take_duration.num_microseconds().unwrap_or(0),
                fire_contract_duration.num_microseconds().unwrap_or(0)
            );

            println!(
                "=== finish search route with countract fired {}",
                datetime_to_jst(Utc::now())
            );

            Ok(())
        },
    }
}

pub struct FireFoundRouteContractResult {
    skip_cause_too_soon: bool,
    commited_tx_hash: Option<String>,
    commit_tx_msg: Option<String>,
}

async fn fire_found_route_contract(
    search_route_result: &SearchRouteResult,
    args: ContractArgs,

    node_id: &str,
    account_info: &AccountInfoForMev,

    version: u64,
    swap_resources: &Vec<SwapResource>,
    mev_engine: Arc<RwLock<Engine>>,
    swap_router_pool: &SwapRouterPool,

    aptos_net_data_client: &AptosDataClient,

    dry_run: bool,
) -> Result<FireFoundRouteContractResult> {
    // guard wrong order  from consequently send
    let route_cache_key = search_route_result.as_cache_key();
    {
        let cached_version = mev_engine
            .read()
            .recent_fired_mev_cache
            .get(&route_cache_key);

        if let Some(cached_version) = cached_version {
            warn!(
                "same contract fired in small period. it might be bug current_vesion:{}, cached contact fired at version:{} it might be a status error",
                version, cached_version
            );

            if version > cached_version && version - cached_version > 30 {
                error!(
                    "same contract fired in small period. it might be bug current_vesion:{}, cached contact fired at version:{} it might be a status error .panic",
                    version, cached_version
                );

                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                panic!("same contract fired in small period. it might be bug current_vesion:{}, cached contact fired at version:{} it might be a status error .panic", version, cached_version);
            }
        }
    }

    let last_fired = get_last_mev_fired_at_nano_sec();
    let now_in_nano_sec = Utc::now().timestamp_nanos() as u64;
    let skip_cause_too_soon = if last_fired == 0 {
        false
    } else {
        // skip if multiple contacts called in 1 sec
        if now_in_nano_sec > last_fired && now_in_nano_sec - last_fired < 1_000_000_000 {
            true
        } else {
            false
        }
    };

    let (commited_tx_hash, commit_tx_msg) = if skip_cause_too_soon {
        warn!(
            "skip calling contact cause too soon. current_vesion:{}, cached contact fired at version:{}",
            version, last_fired
        );

        (None, Some("skip cause too soon".to_string()))
    } else {
        if dry_run {
            info!("skip calling contract cause dryrun {}", version);
            (None, Some("skip cause dryrun".to_string()))
        } else {
            match account_info.account_apt_balance {
                Some(account_balance) if account_balance < GUARD_UNDER_LIMIT_BALANCE => (
                    None,
                    Some(format!(
                        "skip cause account amount is under {}, actually {}",
                        GUARD_UNDER_LIMIT_BALANCE, account_balance
                    )),
                ),

                _ => {
                    println!(
                        "---- start call contract {},{}",
                        datetime_to_jst(Utc::now()),
                        version
                    );
                    let (maybe_tx_hash, maybe_error_msg) = call_order_contract::call_contacat(
                        mev_engine.clone(),
                        version,
                        args,
                        search_route_result.gas_unit_price,
                        search_route_result.use_loan,
                        search_route_result.specified_max_gas_amount,
                        aptos_net_data_client,
                    )
                    .await?;

                    println!(
                        "---- finished call contract {}, {}",
                        datetime_to_jst(Utc::now()),
                        version
                    );
                    (maybe_tx_hash, maybe_error_msg)
                },
            }
        }
    };

    set_last_mev_fired_nano_sec(now_in_nano_sec);
    {
        let mut mev_engine_writer = mev_engine.write();
        mev_engine_writer
            .recent_fired_mev_cache
            .insert(route_cache_key, version);
    }

    Ok(FireFoundRouteContractResult {
        skip_cause_too_soon,
        commited_tx_hash,
        commit_tx_msg,
    })
}
