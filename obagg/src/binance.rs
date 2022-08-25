use rust_decimal::prelude::*;
use futures_util::StreamExt;
use log::{info, error};
use serde_json;
use std::{error::Error, sync::Arc, collections::BTreeMap};
use tokio::io::AsyncWriteExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio::{sync::{mpsc, Mutex}, time::{sleep, Duration}};
use tonic::Status;

use crate::{config, orderbook::{Level, Summary}, server};

// JRF TODO, turn this into a struct, the data in the struct will be the bids and asks
// since we want this struct to manage its own data we would need to define a new() function
// that creates the struct

pub struct Orderbook {
    pub bids: BTreeMap<Decimal,f64>,
    pub asks: BTreeMap<Decimal,f64>,
}

async fn get_snapshot(
    conf: &config::Server,
    bids_arc: &Arc<Mutex<BTreeMap<Decimal,f64>>>,
    asks_arc: &Arc<Mutex<BTreeMap<Decimal,f64>>>
) -> u64 {
    let api_base = url::Url::parse(&conf.apis.binance.as_str()).unwrap();
    let api_channel = format!("/api/v3/depth?symbol={}&limit={}", &conf.ticker.to_uppercase(), 5000);
    let api_url = api_base
        .join(api_channel.as_str())
        .unwrap();
    let snapshot = reqwest::get(api_url)
        .await.unwrap()
        .text()
        .await.unwrap();

    let snapshot: serde_json::Value = serde_json::from_str(&snapshot).expect("Can't parse to JSON");
    {
        let mut bids = bids_arc.lock().await;
        let mut asks = asks_arc.lock().await;
        bids.clear();
        asks.clear();
        let bids_snapshot = snapshot["bids"].as_array().expect("Can't parse snapshot.bids");
        let mut bids_iter = bids_snapshot.into_iter(); //.take(conf.depth.into());

        while let Some(bid) = bids_iter.next() {
            bids.insert(
                bid[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap(),
                bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
            );
        }
        let asks_snapshot = snapshot["asks"].as_array().expect("Can't parse snapshot.bids");
        let mut asks_iter = asks_snapshot.into_iter(); //.take(conf.depth.into());

        while let Some(ask) = asks_iter.next() {
            asks.insert(
                ask[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap(),
                ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
            );
        }
        snapshot["lastUpdateId"].as_u64().expect("lastUpdateId missing from snapshot json.")
    }
}

pub async fn consume_orderbooks(
    conf: &config::Server,
    tx: mpsc::Sender<Result<server::Orderbook, Status>>
) -> Result<(), Box<dyn Error>> {
    // Binance requires that the ticker and params be specified in the url. First we must construct
    // the url.
    let bids_arc = Arc::new(Mutex::new(BTreeMap::new()));
    let asks_arc = Arc::new(Mutex::new(BTreeMap::new()));

    let ws_base = url::Url::parse(&conf.websockets.binance.as_str()).unwrap();
    let ws_channel = format!("/ws/{}@depth@1000ms",&conf.ticker);
    let ws_url = ws_base
        .join(ws_channel.as_str())
        .unwrap();

    info!("Binance Collector Started, attempting to connect to websocket server...");

    let (ws_stream, _) = connect_async(ws_url).await.expect("Failed to connect!");
    println!("Binance WebSocket handshake has been successfully completed.");
    let (_, read) = ws_stream.split();

    // get the snapshot
    let last_update_id_arc =  Arc::new(Mutex::new(get_snapshot(conf, &bids_arc, &asks_arc).await));
    // let bids_arc2 = bids_arc.clone();
    // let asks_arc2 = asks_arc.clone();
    let is_first = Arc::new(Mutex::new(true));
    let prev_u = Arc::new(Mutex::new(0));
    // now that we have the order_book snapshot, we can process updates
    let read_future = {
        read.for_each(|message| async {
            // let data = message.unwrap().into_data();
            // tokio::io::stdout().write_all(&data).await.unwrap();
            {
                let mut is_first_lk = is_first.lock().await;
                let mut prev_u_lk = prev_u.lock().await;
                let mut last_update_id = last_update_id_arc.lock().await;
                let msg = match message.expect("Data was not message!") {
                    Message::Text(s) => { s }
                    _ => {
                        error!("Websocket message was not a string!");
                        *last_update_id = get_snapshot(conf, &bids_arc, &asks_arc).await;
                        *is_first_lk = true;
                        return;
                    }
                };
                let parsed: serde_json::Value = serde_json::from_str(&msg).expect("Can't parse to JSON");
                let mut bids = bids_arc.lock().await;
                let mut asks = asks_arc.lock().await;
                let mut skip = false;
                if let (Some(u), Some(U)) = (parsed["u"].as_u64(), parsed["U"].as_u64()) {
                    // info!(" u = {}, last = {} ", u, *last_update_id);
                    // The first processed event should have U <= lastUpdateId+1 AND u >= lastUpdateId+1.
                    // While listening to the stream, each new event's U should be equal to the previous event's u+1.
                    if u <= *last_update_id {
                        info!("event.u <= last_update_id");
                        return;
                    }
                    info!("prev u {}, curr u {}, curr U {}", *prev_u_lk, u, U);
                    if !*is_first_lk && *prev_u_lk + 1 != U {
                        info!("event.U not equal to previous event u!!!");
                        *last_update_id = get_snapshot(conf, &bids_arc, &asks_arc).await;
                        *is_first_lk = true;
                        return;
                    }

                    if *is_first_lk && U <= *last_update_id + 1 && u >= *last_update_id + 1 {
                        *is_first_lk = false;
                    } else if *is_first_lk {
                        *last_update_id = get_snapshot(conf, &bids_arc, &asks_arc).await;
                        *is_first_lk = true;
                        return;
                    }

                    *prev_u_lk = u;
                    // info!(" u test passed! msg = {}", parsed);
                    // info!("hash map size: bids {}, asks {}",bids.len(), asks.len());
                    let bids_temp = bids.clone();
                    let asks_temp = asks.clone();
                    if let Some(bids_in) = parsed["b"].as_array() {
                        let mut bids_iter = bids_in.into_iter().take(conf.depth.into());
                        while let Some(bid) = bids_iter.next() {
                            let level = bid[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap();
                            let amount = bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap();
                            if let Some(rv) = bids.remove(&level) {
                                // info!("removed bid level that contained {} update was {} @ {}", rv, amount, level);
                            }
                            // info!("about to check amount for bids : {}", amount);
                            if amount > 0.0 {
                                // info!("checked amount for bids");
                                if let None = bids.insert(level, amount) {
                                    // info!("insert bid level {} @ {}", amount, level);
                                }
                                // check if a bid overlaps old ask levels
                                let mut asks_iter = asks_temp.iter();
                                while let Some((l,_)) = asks_iter.next() {
                                    if l <= &level {
                                    info!("asks contains a level from bids update!!! remove them, l = {}, level = {}", l, level);
                                    asks.remove(&l);
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    } else {
                        skip = true;
                    }

                    if let Some(asks_in) = parsed["a"].as_array() {
                        skip = false;
                        let mut asks_iter = asks_in.into_iter().take(conf.depth.into());
                        while let Some(ask) = asks_iter.next() {
                            let level = ask[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap();
                            let amount = ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap();
                            if let Some(rv) = asks.remove(&level) {
                                // info!("removed ask level that contained {} update was {} @ {}", rv, amount, level);
                            }
                            if amount > 0.0 {
                                if let None = asks.insert(level, amount) {
                                    // info!("insert ask level {} @ {}", amount, level);
                                }
                                // check if an ask overlaps old bid levels
                                let mut bids_iter = bids_temp.iter().rev();
                                while let Some((l,_)) = bids_iter.next() {
                                    if l >= &level {
                                        info!("bids contains a level from asks update!!! remove them, l = {}, level = {}", l, level);
                                        bids.remove(&l);
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // else {
                    //     skip = true;
                    // }
                    // info!("hash map size: bids {}, asks {}",bids.len(), asks.len());


                } else {
                    error!("Missing u or U from depth update message.");
                    skip = true;
                }

                if !skip {
                    // info!("hash map size: bids {}, asks {}",bids.len(), asks.len());
                    // JRF TODO, move this into function
                    let mut bids_reduced = bids.clone();
                    let mut asks_reduced = asks.clone();
                    if bids.len() > usize::from(conf.depth) {
                        let bkeys: Vec<&Decimal> = Vec::from_iter(bids.keys());
                        // info!("bkeys:{:#?}",bkeys);
                        let bkey = bkeys[bkeys.len() - usize::from(conf.depth)].clone();
                        bids_reduced = bids_reduced.split_off(&bkey);
                    }
                    if asks.len() > usize::from(conf.depth) {
                        let akeys: Vec<&Decimal> = Vec::from_iter(asks.keys());
                        // info!("akeys:{:#?}",akeys);
                        let akey = akeys[usize::from(conf.depth)].clone();
                        asks_reduced.split_off(&akey);
                    }
                    // info!("hash map size after split: bids {}, asks {}",bids.len(), asks.len());
                    // let orderbook = Orderbook { bids: bids_reduced, asks: asks_reduced };
                    let mut orderbook = server::Orderbook::Binance(Orderbook {
                        bids: bids_reduced,
                        asks: asks_reduced,
                    });
                    // let mut bids_out = Vec::<Level>::with_capacity(conf.depth.into());

                    // let mut asks_out = Vec::<Level>::with_capacity(conf.depth.into());

                    // let mut bids_iter = bids_reduced.iter().rev();
                    // while let Some((level, amount)) = bids_iter.next() {
                    //     bids_out.push(
                    //         Level {
                    //             exchange: "binance".to_string(),
                    //             price: level.to_string().parse::<f64>().unwrap(),
                    //             amount: *amount,
                    //         }
                    //     );
                    // }
                    // let mut asks_iter = asks_reduced.iter();
                    // while let Some((level, amount)) = asks_iter.next() {
                    //     asks_out.push(
                    //         Level {
                    //             exchange: "binance".to_string(),
                    //             price: level.to_string().parse::<f64>().unwrap(),
                    //             amount: *amount,
                    //         }
                    //     );
                    // }
                    // // info!("binance spread = {}", asks_out[0].price - bids_out[0].price);
                    // let item = Summary {
                    //     spread: asks_out[0].price - bids_out[0].price,
                    //     bids: bids_out.clone(),
                    //     asks: asks_out.clone(),
                    // };
                    if let Err(_item) = tx.send(Result::<server::Orderbook, Status>::Ok(orderbook)).await {
                        println!("Error sending bitstamp orderbook item.");
                    };
                }
            }
        })
    };
    read_future.await;
    Ok(())
}
