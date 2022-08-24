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
        info!("snapshot = {}", snapshot["lastUpdateId"]);
        let bids_snapshot = snapshot["bids"].as_array().expect("Can't parse snapshot.bids");
        let mut bids_iter = bids_snapshot.into_iter().take(conf.depth.into());
        info!("FLAG A");

        while let Some(bid) = bids_iter.next() {
            bids.insert(
                bid[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap(),
                bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
            );
        }
        info!("FLAG B");
        let asks_snapshot = snapshot["bids"].as_array().expect("Can't parse snapshot.bids");
        let mut asks_iter = asks_snapshot.into_iter().take(conf.depth.into());
        info!("FLAG 0");
        while let Some(ask) = asks_iter.next() {
            asks.insert(
                ask[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap(),
                ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
            );
        }
        info!("FLAG C");
    }
    // now that we have the order_book snapshot, we can process updates
    let read_future = {
        read.for_each(|message| async {
            // let data = message.unwrap().into_data();
            // tokio::io::stdout().write_all(&data).await.unwrap();
            info!("FLAG D");

            let msg = match message.expect("Data was not message!") {
                Message::Text(s) => { s }
                _ => {
                    error!("Websocket message was not a string!");
                    return;
                }
            };
            info!("FLAG E");
            let parsed: serde_json::Value = serde_json::from_str(&msg).expect("Can't parse to JSON");
            info!("FLAG F");
            let mut bids = bids_arc.lock().await;
            let mut asks = asks_arc.lock().await;
            let mut skip = false;
            info!("FLAG 1");

            if let (Some(u), Some(last_updated_id)) = (parsed["u"].as_u64(), snapshot["lastUpdateId"].as_u64()) {
                info!("FLAG 2");
                if u <= last_updated_id {
                    return;
                }
                info!("FLAG 3");

                if let Some(bids_in) = parsed["b"].as_array() {
                    let mut bids_iter = bids_in.into_iter().take(conf.depth.into());
                    while let Some(bid) = bids_iter.next() {
                        let level = bid[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap();
                        let amount = bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap();
                        if amount > 0.into() {
                            bids.insert(level, amount);
                        } else {
                            bids.remove(&level);
                        }
                        skip = false;
                    }
                } else {
                    skip = true;
                }
                info!("FLAG 4 {}", skip);

                if let Some(asks_in) = snapshot["asks"].as_array() {
                    let mut asks_iter = asks_in.into_iter().take(conf.depth.into());
                    while let Some(ask) = asks_iter.next() {
                        let level = ask[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap();
                        let amount = ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap();
                        if amount > 0.into() {
                            asks.insert(level, amount);
                        } else {
                            asks.remove(&level);
                        }
                        skip = false;
                    }
                } else {
                    skip = true;
                }
                info!("FLAG 5 {}", skip);

            } else {
                if skip == false {
                    skip = false
                } else {
                    skip = true;
                }
            }
            info!("FLAG 6 {}", skip);

            let mut bids_out = Vec::<Level>::with_capacity(conf.depth.into());
            let mut asks_out = Vec::<Level>::with_capacity(conf.depth.into());

            if !skip {
                let mut bids_iter = bids.iter().rev();
                while let Some((level, amount)) = bids_iter.next() {
                    bids_out.push(
                        Level {
                            exchange: "binance".to_string(),
                            price: level.to_string().parse::<f64>().unwrap(),
                            amount: *amount,
                        }
                    );
                }
                let mut asks_iter = asks.iter().rev();
                while let Some((level, amount)) = asks_iter.next() {
                    asks_out.push(
                        Level {
                            exchange: "binance".to_string(),
                            price: level.to_string().parse::<f64>().unwrap(),
                            amount: *amount,
                        }
                    );
                }
                info!("binance spread = {}", asks_out[0].price - bids_out[0].price);
                let item = Summary {
                    spread: asks_out[0].price - bids_out[0].price,
                    bids: bids_out,
                    asks: asks_out,
                };
                if let Err(_item) = tx.send(Result::<Summary, Status>::Ok(item.clone())).await {
                    println!("Error sending bitstamp orderbook item.");
                };
            }
        })
    };
    read_future.await;
    Ok(())
}
