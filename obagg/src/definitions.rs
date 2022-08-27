use rust_decimal::Decimal;
use serde::{self, Deserialize};
use std::collections::BTreeMap;

use crate::orderbook::Level;

pub fn hash_key_offset() -> Decimal {
    Decimal::new(10000000000000000, 0)
}

#[derive(Clone, Debug)]
pub struct AggregatedOrderbook {
    pub bids: BTreeMap<Decimal, Level>,
    pub asks: BTreeMap<Decimal, Level>,
}

impl AggregatedOrderbook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Orderbook {
    pub bids: BTreeMap<Decimal, Level>,
    pub asks: BTreeMap<Decimal, Level>,
}

impl Orderbook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }
}

pub enum Orderbooks {
    Binance(Orderbook),
    Bitstamp(Orderbook),
}

impl Orderbooks {
    pub fn bids(&mut self) -> &mut BTreeMap<Decimal, Level> {
        match self {
            Self::Binance(b) => &mut b.bids,
            Self::Bitstamp(b) => &mut b.bids,
        }
    }

    pub fn asks(&mut self) -> &mut BTreeMap<Decimal, Level> {
        match self {
            Self::Binance(b) => &mut b.asks,
            Self::Bitstamp(b) => &mut b.asks,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BitstampOrderbookMessage {
    pub data: BitstampOrderbookData,
    pub channel: String,
    pub event: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BitstampOrderbookData {
    #[serde(deserialize_with = "crate::serde::u32_from_str")]
    pub timestamp: u32,
    #[serde(deserialize_with = "crate::serde::u64_from_str")]
    pub microtimestamp: u64,
    pub bids: Vec<OrderbookLevel>,
    pub asks: Vec<OrderbookLevel>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BinanceOrderbookMessage {
    pub last_update_id: u64,
    pub bids: Vec<OrderbookLevel>,
    pub asks: Vec<OrderbookLevel>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BinanceOrderbookUpdateMessage {
    #[serde(rename = "e")]
    pub event: String,
    #[serde(rename = "E")]
    pub timestamp: u64,
    #[serde(rename = "s ")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub last_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<OrderbookLevel>,
    #[serde(rename = "a")]
    pub asks: Vec<OrderbookLevel>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct OrderbookLevel {
    #[serde(deserialize_with = "crate::serde::tf64_from_str")]
    pub level: (f64, f64),
}

impl OrderbookLevel {
    pub fn get_level(&self, exchange: &str) -> Level {
        Level {
            amount: self.level.1,
            exchange: exchange.into(),
            price: self.level.0,
        }
    }

    pub fn get_price(&self) -> Decimal {
        self.level.0.to_string().parse::<Decimal>().unwrap()
    }
}
