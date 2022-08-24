use rust_decimal::prelude::*;
use futures_util::StreamExt;
use log::{info, error};
use serde_json;
use std::{error::Error, sync::Arc, collections::BTreeMap};
use tokio::io::AsyncWriteExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio::{sync::{mpsc, Mutex}, time::{sleep, Duration}};
use tonic::Status;

use crate::{ config, orderbook::{Level, Summary} };

pub async fn consume_orderbooks(
    conf: &config::Server,
    tx: mpsc::Sender<Result<Summary, Status>>
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
    let api_base = url::Url::parse(&conf.apis.binance.as_str()).unwrap();
    let api_channel = format!("/api/v3/depth?symbol={}&limit={}", &conf.ticker.to_uppercase(), &conf.depth);
    let api_url = api_base
        .join(api_channel.as_str())
        .unwrap();
    info!("Binance Collector Started, attempting to connect to websocket server...");

    let (ws_stream, _) = connect_async(ws_url).await.expect("Failed to connect!");
    println!("Binance WebSocket handshake has been successfully completed.");
    let (_, read) = ws_stream.split();

    // get the snapshot
    // info!("url : {}", api_url);
    let snapshot = reqwest::get(api_url)
    .await?
    .text()
    .await?;

    let snapshot: serde_json::Value = serde_json::from_str(&snapshot).expect("Can't parse to JSON");
    {
        let mut bids = bids_arc.lock().await;
        let mut asks = asks_arc.lock().await;
        let bids_snapshot = snapshot["bids"].as_array().expect("Can't parse snapshot.bids");
        let mut bids_iter = bids_snapshot.into_iter().take(conf.depth.into());

        while let Some(bid) = bids_iter.next() {
            bids.insert(
                bid[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap(),
                bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
            );
        }
        let asks_snapshot = snapshot["asks"].as_array().expect("Can't parse snapshot.bids");
        let mut asks_iter = asks_snapshot.into_iter().take(conf.depth.into());

        while let Some(ask) = asks_iter.next() {
            asks.insert(
                ask[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap(),
                ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
            );
        }
    }
    // now that we have the order_book snapshot, we can process updates
    let read_future = {
        read.for_each(|message| async {
            // let data = message.unwrap().into_data();
            // tokio::io::stdout().write_all(&data).await.unwrap();

            let msg = match message.expect("Data was not message!") {
                Message::Text(s) => { s }
                _ => {
                    error!("Websocket message was not a string!");
                    return;
                }
            };
            let parsed: serde_json::Value = serde_json::from_str(&msg).expect("Can't parse to JSON");
            let mut bids = bids_arc.lock().await;
            let mut asks = asks_arc.lock().await;
            let mut skip = false;

            if let (Some(u), Some(last_updated_id)) = (parsed["u"].as_u64(), snapshot["lastUpdateId"].as_u64()) {
                // info!(" u = {}, last = {} ", u, last_updated_id);
                if u <= last_updated_id {
                    return;
                }
                // info!(" u test passed! msg = {}", parsed);
                if let Some(bids_in) = parsed["b"].as_array() {
                    let mut bids_iter = bids_in.into_iter().take(conf.depth.into());
                    while let Some(bid) = bids_iter.next() {
                        let level = bid[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap();
                        let amount = bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap();
                        if let Some(rv) = bids.remove(&level) {
                            // info!("removed bid level {} @ {}", rv, level);
                        }
                        info!("about to check amount for bids : {}", amount);
                        if amount > 0.0 {
                            info!("checked amount for bids");
                            if let None = bids.insert(level, amount) {
                                // info!("insert bid level {} @ {}", amount, level);
                            }
                        }
                        skip = false;
                    }
                } else {
                    skip = true;
                }

                if let Some(asks_in) = parsed["a"].as_array() {
                    let mut asks_iter = asks_in.into_iter().take(conf.depth.into());
                    while let Some(ask) = asks_iter.next() {
                        let level = ask[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap();
                        let amount = ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap();
                        if let Some(rv) = asks.remove(&level) {
                            // info!("removed ask level {} @ {}", rv, level);
                        }
                        if amount > 0.0 {
                            if let None = asks.insert(level, amount) {
                                // info!("insert bid level {} @ {}", amount, level);
                            }
                        }
                        skip = false;
                    }
                } else {
                    skip = true;
                }

            } else {
                if skip == false {
                    skip = false
                } else {
                    skip = true;
                }
            }
            info!("hash map size: bids {}, asks {}",bids.len(), asks.len());
            let mut bids_reduced = bids.clone();
            let mut asks_reduced = asks.clone();
            if bids.len() > usize::from(conf.depth) {
                let bkeys: Vec<&Decimal> = Vec::from_iter(bids.keys());
                let bkey = bkeys[bkeys.len() - usize::from(conf.depth)].clone();
                bids_reduced = bids_reduced.split_off(&bkey);
            }
            if asks.len() > usize::from(conf.depth) {
                let akeys: Vec<&Decimal> = Vec::from_iter(asks.keys());
                let akey = akeys[usize::from(conf.depth)].clone();
                asks_reduced.split_off(&akey);
            }
            info!("hash map size after split: bids {}, asks {}",bids.len(), asks.len());

            let mut bids_out = Vec::<Level>::with_capacity(conf.depth.into());

            let mut asks_out = Vec::<Level>::with_capacity(conf.depth.into());

            if !skip {
                let mut bids_iter = bids_reduced.iter().rev();
                while let Some((level, amount)) = bids_iter.next() {
                    bids_out.push(
                        Level {
                            exchange: "binance".to_string(),
                            price: level.to_string().parse::<f64>().unwrap(),
                            amount: *amount,
                        }
                    );
                }
                let mut asks_iter = asks_reduced.iter();
                while let Some((level, amount)) = asks_iter.next() {
                    asks_out.push(
                        Level {
                            exchange: "binance".to_string(),
                            price: level.to_string().parse::<f64>().unwrap(),
                            amount: *amount,
                        }
                    );
                }
                // info!("binance spread = {}", asks_out[0].price - bids_out[0].price);
                let item = Summary {
                    spread: asks_out[0].price - bids_out[0].price,
                    bids: bids_out.clone(),
                    asks: asks_out.clone(),
                };
                if let Err(_item) = tx.send(Result::<Summary, Status>::Ok(item)).await {
                    println!("Error sending bitstamp orderbook item.");
                };
            }
        })
    };
    read_future.await;
    Ok(())
}
