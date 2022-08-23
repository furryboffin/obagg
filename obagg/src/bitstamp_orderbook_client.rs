use futures::{StreamExt, SinkExt};
use log::{info, error};
use serde_json;
use std::{error::Error, sync::Arc};
use tokio::{sync::{mpsc, Mutex}, time::{sleep, Duration}};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::Status;
// use crate::{ config, error::Error, orderbook::{Level, Summary} };
use crate::{ config, orderbook::{Level, Summary} };

// use std::pin::Pin;
// use serde_json::Value;
// use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};

// use futures_core::Stream;
// use tokio_stream::{Stream, wrappers::ReceiverStream};
// use tonic::{transport::Server, Request, Response, Status};

// pub mod orderbook {
//     tonic::include_proto!("orderbook");
// }

pub async fn bitstamp_orderbook_client(
    conf: &config::Server,
    asks: Arc<Mutex<Vec<Level>>>,
    bids: Arc<Mutex<Vec<Level>>>,
    tx_pool: Arc<Mutex<Vec::<mpsc::Sender<Result<Summary, Status>>>>>,
) -> Result<(), Box<dyn Error>> {
// pub async fn bitstamp_orderbook_client(conf: &config::Server) -> Result<(), tonic::transport::Error> {
    info!("Bitstamp Collector Started, attempting to connect to websocket server...");
    let url = url::Url::parse(&conf.websockets.bitstamp.as_str())?;
    let (ws_stream, _) = connect_async(url).await?;
    info!("Bitstamp WebSocket handshake has been successfully completed.");

    let (mut write, read) = ws_stream.split();

    // send json to ws to select channel
    let buf = "{\"event\":\"bts:subscribe\",\"data\":{\"channel\":\"order_book_btcusd\"}}";
    write.send(buf.into()).await?;
    let read_future = {
        read.for_each(|message| async {
            // let data = message.unwrap().into_data();
            let msg = match message.expect("Data was not message!") {
                Message::Text(s) => { s }
                _ => {
                    error!("Websocket message was not a string!");
                    return;
                    // panic!()
                }
            };
            let parsed: serde_json::Value = serde_json::from_str(&msg).expect("Can't parse to JSON");

            // s.lock().await.push(data);
            // println!("NEXT BIT OF DATA! : ");
            // println!("{}", parsed["data"]["bids"]);
            let mut asks_lk = asks.lock().await;
            asks_lk.retain(|_| false);
            let mut bids_lk = bids.lock().await;
            bids_lk.retain(|_| false);
            let mut skip = false;
            if let Some(bids_in) = parsed["data"]["bids"].as_array() {
                let mut bids_iter = bids_in.into_iter().take(conf.depth.into());
                while let Some(bid) = bids_iter.next() {
                    bids_lk.push(
                        Level {
                            exchange: "bitstamp".to_string(),
                            price: bid[0].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                            amount: bid[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                        }
                    );
                }
            } else {
                skip = true;
            }
            if let Some(asks_in) = parsed["data"]["asks"].as_array() {
                let mut asks_iter = asks_in.into_iter().take(conf.depth.into());
                while let Some(ask) = asks_iter.next() {
                    asks_lk.push(
                        Level {
                            exchange: "bitstamp".to_string(),
                            price: ask[0].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                            amount: ask[1].as_str().unwrap().to_string().parse::<f64>().unwrap(),
                        }
                    );
                }
            } else {
                skip = true;
            }
            if !skip {
                let item = Summary {
                    spread: asks_lk[0].price - bids_lk[0].price,
                    bids: (*bids_lk).clone(),
                    asks: (*asks_lk).clone(),
                };
                while tx_pool.lock().await.len() == 0 {
                    sleep(Duration::from_millis(100)).await;
                }
                {
                    let tx_pool_locked = tx_pool.lock().await;
                    let mut tx_pool_iter = tx_pool_locked.iter();
                    let mut futures = vec![];

                    while let Some(tx) = tx_pool_iter.next() {
                        futures.push(tx.send(Result::<Summary, Status>::Ok(item.clone())));
                    }
                    let mut i = 0;
                    for r in futures::future::join_all(futures).await {
                        if let Err(_item) = r {
                            println!("Error sending aggregated orderbook item : {:?}", i);
                        }
                        i += 1;
                    }
                }

                info!("tx_pool length : {}", tx_pool.lock().await.len());
            }
            // tokio::io::stdout().write_all(&data.iter().map(|&x| x.timestamp).collect::<Vec<_>>()).await.unwrap();
        })
    };
    read_future.await;
    Ok(())
}
