pub use client::client;
pub use server::server;
pub use config as lib_config;
pub mod orderbook {
    tonic::include_proto!("orderbook");
}

mod aggregator;
mod binance;
mod bitstamp;
mod client;
pub mod config;
mod definitions;
mod grpc;
mod serde;
mod server;
mod utils;

