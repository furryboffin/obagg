use rust_decimal::Decimal;
use std::{
    collections::{HashMap},
    error::Error,
    sync::Arc,
};
use tokio::{
    sync::{mpsc, Mutex},
    time::{sleep, Duration},
};
use tonic::Status;
use uuid::Uuid;

use crate::{
    config,
    orderbook::{Level, Summary},
    server::{AggregatedOrderbook, Orderbook, Orderbooks},
};

pub async fn aggregate_orderbooks(
    conf: &config::Server,
    rx: &mut mpsc::Receiver<Result<Orderbooks, Status>>,
    tx_pool: Arc<Mutex<HashMap<Uuid, mpsc::Sender<Result<Summary, Status>>>>>,
) -> Result<(), Box<dyn Error>> {
    // let bids_arc = Arc::new(Mutex::new(BTreeMap::new()));
    // let asks_arc = Arc::new(Mutex::new(BTreeMap::new()));
    let mut binance_ob_cache = Orderbook::new();
    let mut bitstamp_ob_cache = Orderbook::new();

    while let Some(msg) = rx.recv().await {
        if let Ok(orderbook) = msg {
            // wait until we have clients connected.
            if tx_pool.lock().await.len() == 0 {
                sleep(Duration::from_millis(100)).await;
                continue;
            }

            let mut aggregated_orderbook = AggregatedOrderbook::new();
            // let mut aggregated_asks: BTreeMap<Decimal, Level> = BTreeMap::new();
            let tx_pool_locked = tx_pool.lock().await;
            let mut tx_pool_iter = tx_pool_locked.iter();
            let mut futures = vec![];

            // match the type of incoming message and cache it in the appropriate book type
            // JRF TODO, figure out how to store the exchange for the cached orderbooks.
            match orderbook {
                Orderbooks::Binance(binance_orderbook) => {
                    binance_ob_cache = binance_orderbook;
                }
                Orderbooks::Bitstamp(bitstamp_orderbook) => {
                    bitstamp_ob_cache = bitstamp_orderbook;
                }
            }

            // aggregate the orderbooks
            let mut binance_bids_iter = binance_ob_cache.bids.iter();
            while let Some((level, amount)) = binance_bids_iter.next() {
                aggregated_orderbook.bids.insert(
                    *level,
                    Level {
                        price: level.to_string().parse::<f64>().unwrap(),
                        amount: *amount,
                        exchange: "binance".to_string(),
                    },
                );
            }

            let mut binance_asks_iter = binance_ob_cache.asks.iter();
            while let Some((level, amount)) = binance_asks_iter.next() {
                aggregated_orderbook.asks.insert(
                    *level,
                    Level {
                        price: level.to_string().parse::<f64>().unwrap(),
                        amount: *amount,
                        exchange: "binance".to_string(),
                    },
                );
            }

            let mut bitstamp_bids_iter = bitstamp_ob_cache.bids.iter();
            while let Some((level, amount)) = bitstamp_bids_iter.next() {
                aggregated_orderbook.bids.insert(
                    *level,
                    Level {
                        price: level.to_string().parse::<f64>().unwrap(),
                        amount: *amount,
                        exchange: "bitstamp".to_string(),
                    },
                );
            }

            let mut bitstamp_asks_iter = bitstamp_ob_cache.asks.iter();
            while let Some((level, amount)) = bitstamp_asks_iter.next() {
                aggregated_orderbook.asks.insert(
                    *level,
                    Level {
                        price: level.to_string().parse::<f64>().unwrap(),
                        amount: *amount,
                        exchange: "bitstamp".to_string(),
                    },
                );
            }

            let mut aggregated_orderbook_reduced = aggregated_orderbook.clone();
            // let mut asks_reduced = aggregated_orderbook.asks.clone();
            if aggregated_orderbook_reduced.bids.len() > usize::from(conf.depth) {
                let bkeys: Vec<&Decimal> = Vec::from_iter(aggregated_orderbook_reduced.bids.keys());
                let bkey = bkeys[bkeys.len() - usize::from(conf.depth)].clone();
                aggregated_orderbook_reduced.bids = aggregated_orderbook_reduced.bids.split_off(&bkey);
            }
            if aggregated_orderbook_reduced.asks.len() > usize::from(conf.depth) {
                let akeys: Vec<&Decimal> = Vec::from_iter(aggregated_orderbook_reduced.asks.keys());
                let akey = akeys[usize::from(conf.depth)].clone();
                aggregated_orderbook_reduced.asks.split_off(&akey);
            }

            // build the Summary
            let bids_out: Vec<Level> = aggregated_orderbook_reduced.bids.into_values().collect();
            let asks_out: Vec<Level> = aggregated_orderbook_reduced.asks.into_values().collect();

            let summary = Summary {
                spread: asks_out[0].price - bids_out[0].price,
                bids: bids_out,
                asks: asks_out,
            };

            // push out the aggregated orderbook to all client tx streams
            while let Some((_, tx)) = tx_pool_iter.next() {
                futures.push(tx.send(Ok(summary.clone())));
            }

            let mut i = 0;
            for r in futures::future::join_all(futures).await {
                if let Err(_item) = r {
                    println!("Error sending aggregated orderbook item : {:?}", i);
                }
                i += 1;
            }
        }
    }
    Ok(())
}