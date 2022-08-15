use serde::Deserialize;
use std::error::Error;
use std::net::SocketAddr;

#[derive(Deserialize)]
pub struct Server {
    pub bind_address: SocketAddr,
}

fn file_from_env(var: &str) -> Result<std::fs::File, Box<dyn Error + Sync + Send>> {
    let path = std::env::var(var)?;
    Ok(std::fs::File::open(path)?)
}

impl Server {
    pub fn from_env() -> Result<Self, Box<dyn Error + Sync + Send>> {
        Ok(serde_yaml::from_reader(file_from_env("AGGREGATED_ORDERBOOK_CONFIG")?)?)
    }
}

