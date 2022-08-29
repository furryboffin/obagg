use rust_decimal::Decimal;
use serde::{self, Deserialize};
use std::collections::BTreeMap;

use crate::orderbook::Level;

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

    pub fn reduce(&self, depth: usize) -> Self {
        let mut orderbook_reduced = self.clone();
        if self.bids.len() > usize::from(depth) && self.asks.len() > usize::from(depth) {
            let bkeys: Vec<&Decimal> = Vec::from_iter(self.bids.keys());
            let bkey = bkeys[bkeys.len() - usize::from(depth)].clone();
            orderbook_reduced.bids = orderbook_reduced.bids.split_off(&bkey);

            let akeys: Vec<&Decimal> = Vec::from_iter(self.asks.keys());
            let akey = akeys[usize::from(depth)].clone();
            orderbook_reduced.asks.split_off(&akey);
        }
        orderbook_reduced
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
    #[serde(rename = "s")]
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

    pub fn get_amount(&self) -> f64 {
        self.level.1
    }
}

#[cfg(test)]
mod tests {
    use super::BinanceOrderbookMessage;
    use super::BinanceOrderbookUpdateMessage;
    use super::BitstampOrderbookData;
    use super::BitstampOrderbookMessage;
    use super::OrderbookLevel;

    #[test]
    fn bitstamp_oderbook_message() {
        let json_message = r#"{
                "data":{
                    "timestamp":"1661585367",
                    "microtimestamp":"1661585367425575",
                    "bids":[
                        ["0.00259978","4.35000000"]
                    ],
                    "asks":[
                        ["0.00344831","7.50000000"]
                    ]
                },
                "channel":"order_book_ltcbtc",
                "event":"data"
            }"#;
        let bitstamp_orderbook_message = BitstampOrderbookMessage {
            data: BitstampOrderbookData {
                timestamp: 1661585367,
                microtimestamp: 1661585367425575,
                bids: vec![OrderbookLevel {
                    level: (0.00259978, 4.35000000),
                }],
                asks: vec![OrderbookLevel {
                    level: (0.00344831, 7.50000000),
                }],
            },
            channel: String::from("order_book_ltcbtc"),
            event: String::from("data"),
        };
        let deserialized_orderbook =
            serde_json::from_str::<BitstampOrderbookMessage>(&json_message).unwrap();
        assert_eq!(deserialized_orderbook, bitstamp_orderbook_message);
    }

    #[test]
    fn binance_oderbook_message() {
        let json_message = r#"{
                "lastUpdateId":1661585367,
                "bids":[
                    ["0.00259978","4.35000000"]
                ],
                "asks":[
                    ["0.00344831","7.50000000"]
                ]
            }"#;
        let bitstamp_orderbook_message = BinanceOrderbookMessage {
            last_update_id: 1661585367,
            bids: vec![OrderbookLevel {
                level: (0.00259978, 4.35000000),
            }],
            asks: vec![OrderbookLevel {
                level: (0.00344831, 7.50000000),
            }],
        };
        let deserialized_orderbook =
            serde_json::from_str::<BinanceOrderbookMessage>(&json_message).unwrap();
        assert_eq!(deserialized_orderbook, bitstamp_orderbook_message);
    }

    #[test]
    fn binance_oderbook_update_message() {
        let json_message = r#"{
                "e":"depthUpdate",
                "E":1661586147639,
                "s":"LTCBTC",
                "U":1753501212,
                "u":1753501215,
                "b":[["0.00259978","4.35000000"]],
                "a":[["0.00344831","7.50000000"]]
            }"#;
        let binance_orderbook_update_message = BinanceOrderbookUpdateMessage {
            event: String::from("depthUpdate"),
            timestamp: 1661586147639,
            symbol: String::from("LTCBTC"),
            first_update_id: 1753501212,
            last_update_id: 1753501215,
            bids: vec![OrderbookLevel {
                level: (0.00259978, 4.35000000),
            }],
            asks: vec![OrderbookLevel {
                level: (0.00344831, 7.50000000),
            }],
        };
        let deserialized_orderbook =
            serde_json::from_str::<BinanceOrderbookUpdateMessage>(&json_message).unwrap();
        assert_eq!(deserialized_orderbook, binance_orderbook_update_message);
    }
}
