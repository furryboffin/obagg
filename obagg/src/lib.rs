pub use client::client;
pub use server::server;
mod orderbook {
    tonic::include_proto!("orderbook");
}

mod aggregator;
mod binance;
mod bitstamp;
mod client;
pub mod config;
mod definitions;
mod error;
mod grpc;
mod serde;
mod server;
mod utils;
