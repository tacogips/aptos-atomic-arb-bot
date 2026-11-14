use anyhow::Result;
use aptos_rest_client;
use aptos_rest_client::Account;
use aptos_storage_interface::{
    state_view::{DbStateView, LatestDbStateCheckpointView},
    DbReader,
};
use aptos_types::account_address::AccountAddress;
use std::sync::Arc;
use url::Url;

pub async fn get_account_via_rpc(
    account_address: AccountAddress,
    rest_api_url: &Url,
) -> Result<Account> {
    let aptos_rest_cli = aptos_rest_client::Client::new(rest_api_url.clone());

    let account = aptos_rest_cli
        .get_account(account_address)
        .await?
        .into_inner();
    Ok(account)
}
