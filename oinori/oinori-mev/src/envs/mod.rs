use anyhow::Result;
use std::env;
use url::Url;

const DEFAULT_MAINNET_RPC: &str = "https://fullnode.mainnet.aptoslabs.com";
const DEFAULT_METRICS_URL: &str = "http://localhost:9023";

#[derive(Clone)]
pub struct OinoriEnvs {
    pub oinori_sender_set_id: String,
    pub rpc_url: Url,
    pub self_metrics_url: String,
    pub contract_dir: String,
    pub node_id: String,
    pub oinori_mev_disable: bool,
    pub oinori_mev_dry_run: bool,
}

impl OinoriEnvs {
    pub fn load() -> Result<Self> {
        let oinori_mev_disable = env::var("OINORI_MEV_DISABLED")
            .map(|value| value == "1")
            .unwrap_or(false);

        let oinori_mev_dry_run = env::var("OINORI_MEV_DRY_RUN")
            .map(|value| value == "1")
            .unwrap_or(false);

        // file at /resources/sender_set/{oinori_sender_set_id}.toml will be loaded
        let oinori_sender_set_id = env::var("OINORI_ACCOUNT_SET_ID").unwrap_or(format!("default"));
        let rpc_url = env::var("OINORI_RPC_URL").unwrap_or(DEFAULT_MAINNET_RPC.to_string());
        let self_metrics_url =
            env::var("OINORI_METRICS_URL").unwrap_or(DEFAULT_METRICS_URL.to_string());

        let node_id = env::var("OINORI_NODE_ID").unwrap_or("default_node".to_string());

        // there should be Move.toml file at
        // {OINORI_CONTRACT_DIR}/contracts/{contract_name}/Move.toml
        let contract_dir = env::var("OINORI_CONTRACT_DIR").unwrap_or_else(|_| {
            format!("{}/contracts", std::env::var("CARGO_MANIFEST_DIR").unwrap())
        });

        Ok(OinoriEnvs {
            oinori_mev_disable,
            oinori_mev_dry_run,
            oinori_sender_set_id,
            rpc_url: Url::parse(&rpc_url)?,
            self_metrics_url,
            contract_dir,
            node_id,
        })
    }
}
