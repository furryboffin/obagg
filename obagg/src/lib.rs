mod server;
mod client;

mod config;
mod bitstamp_orderbook_client;
mod binance_orderbook_client;
pub mod orderbook {
    tonic::include_proto!("orderbook");
}

pub use server::server;
pub use client::client;
#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Error {}
