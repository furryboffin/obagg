use futures_util::StreamExt;
use log::{error, info};
use rust_decimal::prelude::*;
use serde_json;
use std::{error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::Status;

use crate::{
    config,
    server::{Orderbook, Orderbooks},
};

// Get a snapshot of the orderbook from the binance API server. This async function returns a promise
// that resolves to the lastUpdateId returned with the orderbook data. The bids and asks are stored
// in two BTreeMaps passed in
async fn get_snapshot(conf: &config::Server, orderbook: &mut Orderbook) -> u64 {
    let api_base = url::Url::parse(&conf.exchanges.binance.api.as_str()).unwrap();
    let api_channel = format!(
        "/api/v3/depth?symbol={}&limit={}",
        &conf.ticker.to_uppercase(),
        5000
    );
    let api_url = api_base.join(api_channel.as_str()).unwrap();
    let snapshot = reqwest::get(api_url).await.unwrap().text().await.unwrap();

    let snapshot: serde_json::Value = serde_json::from_str(&snapshot).expect("Can't parse to JSON");
    {
        // let mut orderbook.bids = bids_arc.lock().await;
        orderbook.bids.clear();
        orderbook.asks.clear();
        let bids_snapshot = snapshot["bids"]
            .as_array()
            .expect("Can't parse snapshot.bids");
        let mut bids_iter = bids_snapshot.into_iter(); //.take(conf.depth.into());

        while let Some(bid) = bids_iter.next() {
            orderbook.bids.insert(
                bid[0]
                    .as_str()
                    .unwrap()
                    .to_string()
                    .parse::<Decimal>()
                    .unwrap(),
                bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
            );
        }
        let asks_snapshot = snapshot["asks"]
            .as_array()
            .expect("Can't parse snapshot.bids");
        let mut asks_iter = asks_snapshot.into_iter(); //.take(conf.depth.into());

        while let Some(ask) = asks_iter.next() {
            orderbook.asks.insert(
                ask[0]
                    .as_str()
                    .unwrap()
                    .to_string()
                    .parse::<Decimal>()
                    .unwrap(),
                ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
            );
        }
        snapshot["lastUpdateId"]
            .as_u64()
            .expect("lastUpdateId missing from snapshot json.")
    }
}

// Open a websocket connection and process the update messages into a locally stored orderbook.
// the following set of rules are applied:
// 1. Open a stream to the websocket e.g.: wss://stream.binance.com:9443/ws/bnbbtc@depth.
// 2. Buffer the events you receive from the stream.
// 3. Get a depth snapshot from https://api.binance.com/api/v3/depth?symbol=BNBBTC&limit=1000 .
// 4. Drop any event where u is <= lastUpdateId in the snapshot.
// 5. The first processed event should have U <= lastUpdateId+1 AND u >= lastUpdateId+1.
// 6. While listening to the stream, each new event's U should be equal to the previous event's u+1.
// 7. The data in each event is the absolute quantity for a price level.
// 8. If the quantity is 0, remove the price level.
// 9. Receiving an event that removes a price level that is not in your local order book can happen
//    and is normal.
// 10. Remove levels if a new level update from the other side of the book, crosses the level. (note
//     that this rule was not elaborated in the binance documentation.)
pub async fn consume_orderbooks(
    conf: &config::Server,
    tx: mpsc::Sender<Result<Orderbooks, Status>>,
) -> Result<(), Box<dyn Error>> {

    // Binance requires that the ticker and params be specified in the url. First we must construct
    // the url.
    let orderbook_arc = Arc::new(Mutex::new(Orderbook::new()));
    let ws_base = url::Url::parse(&conf.exchanges.binance.websocket.as_str()).unwrap();
    let ws_channel = format!("/ws/{}@depth@1000ms", &conf.ticker);
    let ws_url = ws_base.join(ws_channel.as_str()).unwrap();

    info!("Binance Collector Started, attempting to connect to websocket server...");
    let (ws_stream, _) = connect_async(ws_url).await.expect("Failed to connect!");
    info!("Binance WebSocket handshake has been successfully completed.");
    let (_, read) = ws_stream.split();

    // get the snapshot
    let last_update_id_arc = Arc::new(Mutex::new(
        get_snapshot(conf, &mut *orderbook_arc.lock().await).await,
    ));
    let is_first = Arc::new(Mutex::new(true));
    let prev_u = Arc::new(Mutex::new(0));

    // now that we have the order_book snapshot, we can process updates
    let read_future = {
        read.for_each(|message| async {
            {
                let mut is_first_lk = is_first.lock().await;
                let mut prev_u_lk = prev_u.lock().await;
                let mut last_update_id = last_update_id_arc.lock().await;
                let mut orderbook = orderbook_arc.lock().await;

                let msg = match message.expect("Data was not message!") {
                    Message::Text(s) => s,
                    _ => {
                        error!("Websocket message was not a string!");
                        *last_update_id = get_snapshot(conf, &mut orderbook).await;
                        *is_first_lk = true;
                        return;
                    }
                };
                let parsed: serde_json::Value =
                    serde_json::from_str(&msg).expect("Can't parse to JSON");
                let mut skip = true;
                if let (Some(start_u), Some(end_u)) = (parsed["u"].as_u64(), parsed["U"].as_u64()) {
                    if start_u <= *last_update_id {
                        return;
                    }
                    if !*is_first_lk && *prev_u_lk + 1 != end_u {
                        *last_update_id = get_snapshot(conf, &mut orderbook).await;
                        *is_first_lk = true;
                        return;
                    }

                    if *is_first_lk && end_u <= *last_update_id + 1 && start_u >= *last_update_id + 1 {
                        *is_first_lk = false;
                    } else if *is_first_lk {
                        *last_update_id = get_snapshot(conf, &mut orderbook).await;
                        *is_first_lk = true;
                        return;
                    }

                    *prev_u_lk = start_u;
                    let orderbook_temp = orderbook.clone();
                    if let Some(bids_in) = parsed["b"].as_array() {
                        skip = false;
                        let mut bids_iter = bids_in.into_iter().take(conf.depth.into());
                        while let Some(bid) = bids_iter.next() {
                            let level = bid[0]
                                .as_str()
                                .unwrap()
                                .to_string()
                                .parse::<Decimal>()
                                .unwrap();
                            let amount =
                                bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap();
                            orderbook.bids.remove(&level);
                            if amount > 0.0 {
                                orderbook.bids.insert(level, amount);

                                // check if a bid overlaps old ask levels
                                let mut asks_iter = orderbook_temp.asks.iter();
                                while let Some((l, _)) = asks_iter.next() {
                                    if l <= &level {
                                        orderbook.asks.remove(&l);
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if let Some(asks_in) = parsed["a"].as_array() {
                        skip = false;
                        let mut asks_iter = asks_in.into_iter().take(conf.depth.into());
                        while let Some(ask) = asks_iter.next() {
                            let level = ask[0]
                                .as_str()
                                .unwrap()
                                .to_string()
                                .parse::<Decimal>()
                                .unwrap();
                            let amount =
                                ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap();
                            orderbook.asks.remove(&level);
                            if amount > 0.0 {
                                orderbook.asks.insert(level, amount);

                                // check if an ask overlaps old bid levels
                                let mut bids_iter = orderbook_temp.bids.iter().rev();
                                while let Some((l, _)) = bids_iter.next() {
                                    if l >= &level {
                                        orderbook.bids.remove(&l);
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                } else {
                    error!("Missing u or U from depth update message.");
                    skip = true;
                }

                if !skip {
                    // JRF TODO, move this into function
                    let mut orderbook_reduced = orderbook.clone();
                    if orderbook.bids.len() > usize::from(conf.depth) {
                        let bkeys: Vec<&Decimal> = Vec::from_iter(orderbook.bids.keys());
                        // info!("bkeys:{:#?}",bkeys);
                        let bkey = bkeys[bkeys.len() - usize::from(conf.depth)].clone();
                        orderbook_reduced.bids = orderbook_reduced.bids.split_off(&bkey);
                    }
                    if orderbook.asks.len() > usize::from(conf.depth) {
                        let akeys: Vec<&Decimal> = Vec::from_iter(orderbook.asks.keys());
                        // info!("akeys:{:#?}",akeys);
                        let akey = akeys[usize::from(conf.depth)].clone();
                        orderbook_reduced.asks.split_off(&akey);
                    }

                    if let Err(_item) = tx
                        .send(Result::<Orderbooks, Status>::Ok(Orderbooks::Binance(
                            orderbook_reduced,
                        )))
                        .await
                    {
                        println!("Error sending bitstamp orderbook item.");
                    };
                } else {
                    error!("Skipped message.");
                }
            }
        })
    };
    read_future.await;
    Ok(())
}
