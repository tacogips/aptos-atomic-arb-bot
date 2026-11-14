use crate::transaction::builder::*;
use anyhow::Result;
use std::path::PathBuf;

use aptos_types::chain_id::ChainId;

pub fn build_hippo_aggregator_script_v1(
    chain_id: ChainId,
    sender: TransactionSender,
    default_max_gas: Option<u64>,
    contract_dir: &str,
) -> Result<TransactionBase> {
    let build_package_info =
        BuildPackageInfo::new(module_dir(contract_dir, "oinori_swap_v1"), None, false);

    TransactionBase::new_script(
        sender,
        build_package_info,
        Some(chain_id),
        Some(default_max_gas.unwrap_or(5_000)),
    )
}

pub fn build_hippo_aggregator_script_v1_no_loan(
    chain_id: ChainId,
    sender: TransactionSender,
    max_gas: Option<u64>,
    contract_dir: &str,
) -> Result<TransactionBase> {
    let build_package_info = BuildPackageInfo::new(
        module_dir(contract_dir, "oinori_swap_v1_no_loan"),
        None,
        false,
    );

    TransactionBase::new_script(
        sender,
        build_package_info,
        Some(chain_id),
        Some(max_gas.unwrap_or(5_000)),
    )
}

pub fn build_hippo_aggregator_script_v2(
    chain_id: ChainId,
    sender: TransactionSender,
    default_max_gas: Option<u64>,
    contract_dir: &str,
) -> Result<TransactionBase> {
    let build_package_info =
        BuildPackageInfo::new(module_dir(contract_dir, "oinori_swap_v2"), None, false);

    TransactionBase::new_script(
        sender,
        build_package_info,
        Some(chain_id),
        Some(default_max_gas.unwrap_or(5_000)),
    )
}

pub fn build_hippo_aggregator_script_v2_no_loan(
    chain_id: ChainId,
    sender: TransactionSender,
    default_max_gas: Option<u64>,
    contract_dir: &str,
) -> Result<TransactionBase> {
    let build_package_info = BuildPackageInfo::new(
        module_dir(contract_dir, "oinori_swap_v2_no_loan"),
        None,
        false,
    );

    TransactionBase::new_script(
        sender,
        build_package_info,
        Some(chain_id),
        Some(default_max_gas.unwrap_or(5_000)),
    )
}

fn module_dir(contract_dir: &str, contract_name: &str) -> PathBuf {
    let mut p = PathBuf::new();
    p.push(contract_dir);
    p.push(contract_name);
    p
}
