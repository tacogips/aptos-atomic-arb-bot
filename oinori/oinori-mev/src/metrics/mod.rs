use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;

const SYNC_VERSION_LIEN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"aptos_state_sync_version.type="synced". ([0-9]+)"#).unwrap());

pub struct Metrics {
    pub synced_version: u64,
}

pub async fn get_metrics(metrics_host: &str) -> Result<Metrics> {
    let body = reqwest::get(format!("{}/metrics", metrics_host))
        .await?
        .text()
        .await?;

    let mut synced_version = 0;
    for each_line in body.split("\n") {
        if let Some(current_synced_version) = match_synced_version(each_line) {
            synced_version = current_synced_version;
            break;
        }
    }

    Ok(Metrics { synced_version })
}

fn match_synced_version(line: &str) -> Option<u64> {
    match SYNC_VERSION_LIEN_RE.captures(line) {
        Some(cap) => {
            if let Some(cap) = cap.get(1) {
                Some(cap.as_str().parse().unwrap())
            } else {
                None
            }
        }
        None => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_match_synced_version() {
        let line = r#"aptos_state_sync_version{type="executed_transactions"} 78595412"#;
        let actual = match_synced_version(line);
        assert_eq!(actual, None);

        let line = r#"aptos_state_sync_version{type="synced"} 79070267"#;
        let actual = match_synced_version(line);
        assert_eq!(actual, Some(79070267));

        let line = r#"aptos_state_sync_version{type="synced_epoch"} 1316"#;
        let actual = match_synced_version(line);
        assert_eq!(actual, None);
    }
}
