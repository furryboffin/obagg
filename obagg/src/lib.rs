pub use client::client;
pub use server::server;

mod aggregator;
mod binance;
mod bitstamp;
mod client;
mod config;
mod definitions;
mod grpc;
mod server;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}
