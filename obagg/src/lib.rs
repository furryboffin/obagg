pub use server::server;
pub use client::client;

mod binance_orderbook_client;
mod bitstamp_orderbook_client;
mod client;
mod config;
// mod error;
mod server;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

// #[derive(Debug)]
// pub struct Error(String);

// impl std::fmt::Display for Error {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }

// impl std::error::Error for Error {}
