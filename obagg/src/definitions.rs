use rust_decimal::Decimal;
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
