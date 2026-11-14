use anyhow::Result;
use aptos_api_types::{PendingTransaction as ApiPendingTransaction, Transaction as ApiTransaction};
use aptos_rest_client;
use aptos_types::transaction::SignedTransaction;
use url::Url;

pub async fn send_transaction_via_rest_client(
    signed_transaction: &SignedTransaction,
    rest_api_url: &Url,
) -> Result<ApiPendingTransaction> {
    let aptos_rest_cli = aptos_rest_client::Client::new(rest_api_url.clone());
    let response = aptos_rest_cli.submit(signed_transaction).await?;
    Ok(response.into_inner())
}

pub async fn wait_transaction(
    signed_transaction: &SignedTransaction,
    rest_api_url: &Url,
) -> Result<ApiTransaction> {
    let aptos_rest_cli = aptos_rest_client::Client::new(rest_api_url.clone());
    let response = aptos_rest_cli
        .wait_for_signed_transaction(signed_transaction)
        .await?;

    Ok(response.into_inner())
}

