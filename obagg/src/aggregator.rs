
use itertools::Itertools;
use log::info;
use std::collections::{HashMap, BTreeMap};
use std::{error::Error, sync::Arc};
use rust_decimal::Decimal;
use tokio::{sync::{mpsc, Mutex}, time::{sleep, Duration}};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::Status;
use uuid::Uuid;
use std::sync::mpsc::channel;
use std::thread;

use crate::{binance, bitstamp};
use crate::{ config, orderbook::{Level, Summary}, server };

pub async fn aggregate_orderbooks(
    conf: &config::Server,
    rx: &mut mpsc::Receiver<Result<server::Orderbook, Status>>,
    tx_pool: Arc<Mutex<HashMap<Uuid, mpsc::Sender<Result<Summary, Status>>>>>,
) -> Result<(), Box<dyn Error>> {
    // let bids_arc = Arc::new(Mutex::new(BTreeMap::new()));
    // let asks_arc = Arc::new(Mutex::new(BTreeMap::new()));
    let mut binance_ob_cache = binance::Orderbook {
        bids: BTreeMap::new(),
        asks: BTreeMap::new(),
    };
    let mut bitstamp_ob_cache = bitstamp::Orderbook {
        bids: BTreeMap::new(),
        asks: BTreeMap::new(),
    };
    while let Some(msg) = rx.recv().await {
        if let Ok(orderbook) = msg {
            // wait until we have clients connected.
            if tx_pool.lock().await.len() == 0 {
                sleep(Duration::from_millis(100)).await;
                continue;
            }

            let mut aggregated_bids: BTreeMap<Decimal, Level> = BTreeMap::new();
            let mut aggregated_asks: BTreeMap<Decimal, Level> = BTreeMap::new();
            let tx_pool_locked = tx_pool.lock().await;
            let mut tx_pool_iter = tx_pool_locked.iter();
            let mut futures = vec![];
            let mut exchange;

            // match the type of incoming message and cache it in the appropriate book type
            // JRF TODO, figure out how to store the exchange for the cached orderbooks.
            match orderbook {
                server::Orderbook::Binance(binance_orderbook) => {
                    exchange = "binance";
                    binance_ob_cache.bids = binance_orderbook.bids;
                    binance_ob_cache.asks = binance_orderbook.asks;
                },
                server::Orderbook::Bitstamp(bitstamp_orderbook) => {
                    exchange = "bitstamp";
                    bitstamp_ob_cache.bids = bitstamp_orderbook.bids;
                    bitstamp_ob_cache.asks = bitstamp_orderbook.asks;
                },
            }
            // if
            //     binance_ob_cache.bids.len() == 0 ||
            //     binance_ob_cache.asks.len() == 0 ||
            //     bitstamp_ob_cache.bids.len() == 0 ||
            //     bitstamp_ob_cache.asks.len() == 0 {
            //         continue;
            // }

            info!("binance orderbook.len() bids {} asks {}", binance_ob_cache.bids.len(), binance_ob_cache.asks.len());
            info!("bitstamp orderbook.len() bids {} asks {}", bitstamp_ob_cache.bids.len(), bitstamp_ob_cache.asks.len());
            // aggregate the orderbooks
            let mut binance_bids_iter = binance_ob_cache.bids.iter();
            while let Some((level, amount)) = binance_bids_iter.next() {
                aggregated_bids.insert(*level, Level{ price: level.to_string().parse::<f64>().unwrap(), amount: *amount, exchange: "binance".to_string()});
            }
            let mut binance_asks_iter = binance_ob_cache.asks.iter();
            while let Some((level, amount)) = binance_asks_iter.next() {
                aggregated_asks.insert(*level, Level{ price: level.to_string().parse::<f64>().unwrap(), amount: *amount, exchange: "binance".to_string()});
            }
            // aggregated_bids = binance_ob_cache.bids;
            info!("aggregated orderbook.len() bids {} asks {}", aggregated_bids.len(), aggregated_asks.len());

            let mut bitstamp_bids_iter = bitstamp_ob_cache.bids.iter();
            while let Some((level, amount)) = bitstamp_bids_iter.next() {
                aggregated_bids.insert(*level, Level{ price: level.to_string().parse::<f64>().unwrap(), amount: *amount, exchange: "bitstam".to_string()});
            }
            let mut bitstamp_asks_iter = bitstamp_ob_cache.asks.iter();
            while let Some((level, amount)) = bitstamp_asks_iter.next() {
                aggregated_asks.insert(*level, Level{ price: level.to_string().parse::<f64>().unwrap(), amount: *amount, exchange: "bitstam".to_string()});
            }

            // let _ = aggregated_bids.iter().merge_join_by(bitstamp_ob_cache.bids.iter(), |l, a| ( l.0 , l.1 ) = ( a.0, Level{price: a.0, price: a.1, exchange: "bitstamp"}) ::std::cmp::Ordering::Equal);
            // aggregated_asks = binance_ob_cache.asks.clone();
            // let _ = aggregated_asks.iter().merge_join_by(bitstamp_ob_cache.asks.iter(), |_, _| ::std::cmp::Ordering::Equal);
            info!("aggregated orderbook.len() bids {} asks {}", aggregated_bids.len(), aggregated_asks.len());

            let mut bids_reduced = aggregated_bids.clone();
            let mut asks_reduced = aggregated_asks.clone();
            if aggregated_bids.len() > usize::from(conf.depth) {
                let bkeys: Vec<&Decimal> = Vec::from_iter(aggregated_bids.keys());
                // info!("bkeys:{:#?}",bkeys);
                let bkey = bkeys[bkeys.len() - usize::from(conf.depth)].clone();
                bids_reduced = bids_reduced.split_off(&bkey);
            }
            if aggregated_asks.len() > usize::from(conf.depth) {
                let akeys: Vec<&Decimal> = Vec::from_iter(aggregated_asks.keys());
                // info!("akeys:{:#?}",akeys);
                let akey = akeys[usize::from(conf.depth)].clone();
                asks_reduced.split_off(&akey);
            }

            // build the Summary
            let mut bids_out = Vec::<Level>::with_capacity(conf.depth.into());
            let mut asks_out = Vec::<Level>::with_capacity(conf.depth.into());

            let mut bids_iter = aggregated_bids.iter().rev();
            while let Some((_, amount)) = bids_iter.next() {
                bids_out.push(amount.clone());
            }
            let mut asks_iter = asks_reduced.iter();
            while let Some((_, amount)) = asks_iter.next() {
                asks_out.push(amount.clone());
            }
            let summary = Summary {
                spread: asks_out[0].price - bids_out[0].price,
                bids: bids_out.clone(),
                asks: asks_out.clone(),
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
