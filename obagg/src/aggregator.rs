use rust_decimal::Decimal;
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::{
    sync::{mpsc, Mutex},
    time::{sleep, Duration},
};
use tonic::Status;
use uuid::Uuid;

use crate::utils;
use crate::{
    config,
    definitions::{AggregatedOrderbook, Orderbook, Orderbooks},
    orderbook::{Level, Summary},
};

pub async fn aggregate_orderbooks(
    conf: &config::Server,
    rx: &mut mpsc::Receiver<Result<Orderbooks, Status>>,
    tx_pool: Arc<Mutex<HashMap<Uuid, mpsc::Sender<Result<Summary, Status>>>>>,
) -> Result<(), Box<dyn Error>> {
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
            let tx_pool_locked = tx_pool.lock().await;
            let mut tx_pool_iter = tx_pool_locked.iter();
            let mut futures = vec![];

            // match the type of incoming message and cache it in the appropriate book type
            // JRF TODO. at this point we need to map the keys to the new key format that includes the amount so that we can get the ordering correct.
            match orderbook {
                Orderbooks::Binance(binance_orderbook) => {
                    binance_ob_cache = binance_orderbook.clone();

                    aggregated_orderbook.bids = bitstamp_ob_cache
                        .bids
                        .clone()
                        .into_iter()
                        .map(|k| utils::map_key(k, conf, true))
                        .collect();

                    aggregated_orderbook.bids.append(
                        &mut binance_orderbook
                            .bids
                            .into_iter()
                            .map(|k| utils::map_key(k, conf, true))
                            .collect(),
                    );

                    aggregated_orderbook.asks = bitstamp_ob_cache
                        .asks
                        .clone()
                        .into_iter()
                        .map(|k| utils::map_key(k, conf, false))
                        .collect();

                    aggregated_orderbook.asks.append(
                        &mut binance_orderbook
                            .asks
                            .into_iter()
                            .map(|k| utils::map_key(k, conf, false))
                            .collect(),
                    );
                }
                Orderbooks::Bitstamp(bitstamp_orderbook) => {
                    bitstamp_ob_cache = bitstamp_orderbook.clone();
                    aggregated_orderbook.bids = binance_ob_cache
                        .bids
                        .clone()
                        .into_iter()
                        .map(|k| utils::map_key(k, conf, true))
                        .collect();
                    aggregated_orderbook.bids.append(
                        &mut bitstamp_orderbook
                            .bids
                            .into_iter()
                            .map(|k| utils::map_key(k, conf, true))
                            .collect(),
                    );
                    aggregated_orderbook.asks = binance_ob_cache
                        .asks
                        .clone()
                        .into_iter()
                        .map(|k| utils::map_key(k, conf, false))
                        .collect();
                    aggregated_orderbook.asks.append(
                        &mut bitstamp_orderbook
                            .asks
                            .into_iter()
                            .map(|k| utils::map_key(k, conf, false))
                            .collect(),
                    );
                }
            }

            let mut aggregated_orderbook_reduced = aggregated_orderbook.clone();

            if aggregated_orderbook_reduced.bids.len() > usize::from(conf.depth) {
                let bkeys: Vec<&Decimal> = Vec::from_iter(aggregated_orderbook_reduced.bids.keys());
                let bkey = bkeys[bkeys.len() - usize::from(conf.depth)].clone();
                aggregated_orderbook_reduced.bids =
                    aggregated_orderbook_reduced.bids.split_off(&bkey);
            }

            if aggregated_orderbook_reduced.asks.len() > usize::from(conf.depth) {
                let akeys: Vec<&Decimal> = Vec::from_iter(aggregated_orderbook_reduced.asks.keys());
                let akey = akeys[usize::from(conf.depth)].clone();
                aggregated_orderbook_reduced.asks.split_off(&akey);
            }

            // build the Summary
            let bids_out: Vec<Level> = aggregated_orderbook_reduced
                .bids
                .into_values()
                .rev()
                .collect();
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
