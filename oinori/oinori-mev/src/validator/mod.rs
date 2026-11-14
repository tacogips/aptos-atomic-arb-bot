use anyhow::{anyhow, Result};
use url::Url;
use urlencoding::encode;

use aptos_config::config::{Peer, PeerRole};
use aptos_types::{account_config::CORE_CODE_ADDRESS, on_chain_config::ValidatorSet};

use aptos_rest_client::Response;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct OinoriValidator {
    pub account_address: String, // == peer_id
    pub peer: Peer,
    pub consensus_voting_power: u64,

    pub ip_or_address: Option<String>,
    pub location: Option<String>,
    pub timezone: Option<String>,
    pub isp: Option<String>,
}
impl OinoriValidator {
    fn address_or_ip(&self) -> Option<String> {
        match self.peer.addresses.get(0).as_ref() {
            None => None,
            Some(address) => match address.find_ip_addr() {
                None => address.find_dns(),
                Some(ip_addr) => Some(ip_addr.to_string()),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub status: String,
    pub country: String,
    pub country_code: String,
    pub region: String,
    pub region_name: String,
    pub city: String,
    pub zip: String,
    pub lat: f64,
    pub lon: f64,
    pub timezone: String,
    pub isp: String,
    pub org: String,
    #[serde(rename = "as")]
    pub as_field: String,
    pub query: String,
}

pub async fn insert_validator_countries(validators: &mut Vec<OinoriValidator>) -> Result<()> {
    for each in validators {
        if let Some(address_or_ip) = each.address_or_ip() {
            if let Ok(location) = get_location(&address_or_ip).await {
                each.location = Some(location.country.clone());
                each.timezone = Some(location.timezone.clone());
                each.isp = Some(location.isp.clone());
                each.ip_or_address = Some(address_or_ip);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
    Ok(())
}

async fn get_location(ip_address_or_host: &str) -> Result<Location> {
    println!("fetching location :http://ip-api.com/json/{ip_address_or_host}");

    let encoded_address = encode(ip_address_or_host);
    let url = format!("http://ip-api.com/json/{encoded_address}");

    let result: Location = reqwest::get(url.to_string()).await?.json().await?;
    Ok(result)
}

pub async fn fetch_validators(rest_url: &str) -> Result<Vec<OinoriValidator>> {
    let client = aptos_rest_client::Client::new(Url::parse(rest_url).unwrap());

    let response: Response<ValidatorSet> = client
        .get_account_resource_bcs(CORE_CODE_ADDRESS, "0x1::stake::ValidatorSet")
        .await?;

    let (peer_addrs, _state) = response.into_parts();

    let validator_peers: Vec<OinoriValidator> = peer_addrs
        .clone()
        .into_iter()
        .filter_map(|validator_info| -> Option<OinoriValidator> {
            let consensus_voting_power = validator_info.consensus_voting_power();
            validator_info
                .config()
                .validator_network_addresses()
                .map(|addresses| {
                    OinoriValidator {
                        account_address: validator_info.account_address().to_hex_literal(), // == peer_id
                        peer: Peer::from_addrs(PeerRole::Validator, addresses.clone()),
                        consensus_voting_power,
                        ip_or_address: None,
                        location: None,
                        timezone: None,
                        isp: None,
                    }
                })
                .map_err(|err| {
                    anyhow!(
                        "unable to parse validator network address for validator info {}",
                        err
                    )
                })
                .ok()
        })
        .collect();
    Ok(validator_peers)
}
