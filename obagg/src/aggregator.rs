use log::error;
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tonic::Status;
use uuid::Uuid;

use crate::utils;
use crate::{
    config,
    definitions::{Orderbook, Orderbooks},
    orderbook::{Level, Summary},
};

pub async fn aggregate_orderbooks(
    conf: &config::Server,
    rx: &mut mpsc::Receiver<Result<Orderbooks, Status>>,
    tx_pool: Arc<Mutex<HashMap<Uuid, mpsc::Sender<Result<Summary, Status>>>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut binance_ob_cache = Orderbook::new();
    let mut bitstamp_ob_cache = Orderbook::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            Ok(orderbook) => {
                let mut aggregated_orderbook = Orderbook::new();

                // match the type of incoming message, cache it in the appropriate book type and
                // aggregate the cache of the other book type into the aggregated_orderbook.
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

                let aggregated_orderbook_reduced = aggregated_orderbook.reduce(conf.depth);


                let tx_pool_locked = tx_pool.lock().await;
                if tx_pool_locked.len() == 0 {
                    continue;
                }
                let mut tx_pool_iter = tx_pool_locked.iter();
                let mut futures = vec![];
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

                for r in futures::future::join_all(futures).await {
                    if let Err(_item) = r {
                        error!("Error sending aggregated orderbook Summary");
                    }
                }
            },
            Err(status) => {
                error!("Input message was not an orderbook : {}", status);
            }
        }
    }
    error!("Input stream closed unexpectedly!");
    Ok(())
}
