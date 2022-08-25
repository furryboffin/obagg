use futures::{StreamExt, SinkExt};
use log::{info, error};
use rust_decimal::Decimal;
use serde_json;
use std::{error::Error, sync::Arc, collections::BTreeMap};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::Status;

// use crate::{ config, error::Error, orderbook::{Level, Summary} };
use crate::{ config, orderbook::{Level, Summary}, server };

// use std::pin::Pin;
// use serde_json::Value;
// use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};

// use futures_core::Stream;
// use tokio_stream::{Stream, wrappers::ReceiverStream};
// use tonic::{transport::Server, Request, Response, Status};

// pub mod orderbook {
//     tonic::include_proto!("orderbook");
// }

pub struct Orderbook {
    pub bids: BTreeMap<Decimal,f64>,
    pub asks: BTreeMap<Decimal,f64>,
}


pub async fn consume_orderbooks(
    conf: &config::Server,
    tx: mpsc::Sender<Result<server::Orderbook, Status>>,
) -> Result<(), Box<dyn Error>> {

    // let bids = Arc::new(Mutex::new(Vec::<Level>::with_capacity(conf.depth.into())));
    // let asks = Arc::new(Mutex::new(Vec::<Level>::with_capacity(conf.depth.into())));
    info!("Bitstamp Collector Started, attempting to connect to websocket server...");
    let url = url::Url::parse(&conf.websockets.bitstamp.as_str())?;
    let (ws_stream, _) = connect_async(url).await?;
    info!("Bitstamp WebSocket handshake has been successfully completed.");
    let (mut write, read) = ws_stream.split();

    // send json to ws to select channel
    let buf = format!("{{\"event\":\"bts:subscribe\",\"data\":{{\"channel\":\"order_book_{}\"}}}}", conf.ticker);
    info!("buf = {}", buf);
    write.send(buf.into()).await?;
    let read_future = {
        read.for_each(|message| async {
            // let mut bids = Vec::<Level>::with_capacity(conf.depth.into());
            // let mut asks = Vec::<Level>::with_capacity(conf.depth.into());
            let mut orderbook = server::Orderbook::Bitstamp(Orderbook {
                bids: BTreeMap::new(),
                asks: BTreeMap::new(),
            });
            // let data = message.unwrap().into_data();
            // JRF TODO, we must handle these exceptions where data was not a message.
            // currently the server panics! Could this be caused by running two different binaries?
            let msg = match message.expect("Data was not message!") {
                Message::Text(s) => { s }
                _ => {
                    error!("Websocket message was not a string!");
                    return;
                }
            };
            let parsed: serde_json::Value = serde_json::from_str(&msg).expect("Can't parse to JSON");

            // println!("NEXT BIT OF DATA! : ");
            // println!("{}", parsed["data"]["bids"]);
            // let mut bids_lk = bids.lock().await;
            // let mut asks_lk = asks.lock().await;
            // bids_lk.retain(|_| false);
            // asks_lk.retain(|_| false);
            let mut skip = true;
            if let Some(bids_in) = parsed["data"]["bids"].as_array() {
                skip = false;
                let mut bids_iter = bids_in.into_iter().take(conf.depth.into());
                while let Some(bid) = bids_iter.next() {
                    orderbook.bids().insert(
                        bid[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap(),
                        bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                    );
                    // bids.push(
                    //     Level {
                    //         exchange: "bitstamp".to_string(),
                    //         price: bid[0].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                    //         amount: bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                    //     }
                    // );
                }
            }
            if let Some(asks_in) = parsed["data"]["asks"].as_array() {
                skip = false;
                let mut asks_iter = asks_in.into_iter().take(conf.depth.into());
                while let Some(ask) = asks_iter.next() {
                    orderbook.asks().insert(
                        ask[0].as_str().unwrap().to_string().parse::<Decimal>().unwrap(),
                        ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                    );
                    // asks.push(
                    //     Level {
                    //         exchange: "bitstamp".to_string(),
                    //         price: ask[0].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                    //         amount: ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                    //     }
                    // );
                }
            }
            if !skip {
                // let item = Summary {
                //     spread: asks[0].price - asks[0].price,
                //     bids,
                //     asks,
                // };
                if let Err(_item) = tx.send(Result::<server::Orderbook, Status>::Ok(orderbook)).await {
                    println!("Error sending bitstamp orderbook item.");
                };
            }
        })
    };
    read_future.await;
    Ok(())
}
