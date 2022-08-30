use serde::{de::DeserializeOwned, Deserialize};
use std::{error::Error, net::SocketAddr};

#[derive(Deserialize)]
pub struct Apis {
    pub binance: String,
}

#[derive(Deserialize, Clone)]
pub struct Exchange {
    pub api: String,
    pub enable: bool,
    pub websocket: String,
    pub ping_period: u16,
    pub period: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct Exchanges {
    pub binance: Exchange,
    pub bitstamp: Exchange,
}

#[derive(Deserialize, Clone)]
pub struct Server {
    pub bind_address: SocketAddr,
    pub depth: usize,
    pub exchanges: Exchanges,
    pub identical_level_order: bool,
    pub ticker: String,
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

pub fn read_config<T: DeserializeOwned>() -> T {
    let file = file_from_env("AGGREGATED_ORDERBOOK_CONFIG")
        .expect("Error when opening file pointed to by AGGREGATED_ORDERBOOK_CONFIG env variable");
    serde_yaml::from_reader(file).expect("Error parsing configuration file!")
}
