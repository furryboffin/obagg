use serde::Deserialize;
use std::{error::Error, net::SocketAddr};

#[derive(Deserialize)]
pub struct Exchange {
    pub enable: bool,
    pub websocket: String,
    pub api: String,
}

#[derive(Deserialize)]
pub struct Exchanges {
    pub binance: Exchange,
    pub bitstamp: Exchange,
}

#[derive(Deserialize)]
pub struct Apis {
    pub binance: String,
}

#[derive(Deserialize)]
pub struct Server {
    pub bind_address: SocketAddr,
    pub depth: u16,
    pub ticker: String,
    pub exchanges: Exchanges,
}

fn file_from_env(var: &str) -> Result<std::fs::File, Box<dyn Error + Sync + Send>> {
    let path = std::env::var(var)?;
    Ok(std::fs::File::open(path)?)
}

impl Server {
    pub fn from_env() -> Result<Self, Box<dyn Error + Sync + Send>> {
        Ok(serde_yaml::from_reader(file_from_env(
            "AGGREGATED_ORDERBOOK_CONFIG",
        )?)?)
    }
}
