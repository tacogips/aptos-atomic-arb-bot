use anyhow::Result;
use aptos_rest_client;
use aptos_rest_client::State;
use url::Url;

pub async fn get_ledger_info_via_rpc(rest_api_url: &Url) -> Result<State> {
    let aptos_rest_cli = aptos_rest_client::Client::new(rest_api_url.clone());

    let ledger_state = aptos_rest_cli.get_ledger_information().await?.into_inner();
    Ok(ledger_state)
}
