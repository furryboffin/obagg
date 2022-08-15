use serde::{Deserialize, Deserializer};
use std::time::Duration;
use url::Url;

pub fn deserialize_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Url::parse(&s).map_err(serde::de::Error::custom)
}

pub fn deserialize_base_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let url = deserialize_url(deserializer)?;
    if url.path() == "/" {
        Ok(url)
    } else {
        Err(serde::de::Error::custom("Path of base URL must be /"))
    }
}

pub fn deserialize_eth_nonce<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let without_prefix = s.trim_start_matches("0x");
    u64::from_str_radix(without_prefix, 16)
        .map_err(|e| serde::de::Error::custom(format!("Invalid nonce value: {}", e)))
}

pub fn deserialize_seconds<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds = u64::deserialize(deserializer)?;
    Ok(Duration::new(seconds, 0))
}
