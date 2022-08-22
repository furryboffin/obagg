use std::sync::Arc;
use tokio::sync::Mutex;

use std::pin::Pin;
use futures::{StreamExt, SinkExt};
use serde_json;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio::sync::mpsc;

// use futures_core::Stream;
use tokio_stream::{Stream, wrappers::ReceiverStream};
use tonic::{transport::Server, Request, Response, Status};
use tokio::time::{sleep, Duration};

use crate::{ config, server::stream_function, orderbook, orderbook::{Empty, Level, Summary} };
// pub mod orderbook {
//     tonic::include_proto!("orderbook");
// }

pub async fn bitstamp_orderbook_client(
    conf: &config::Server,
    asks: Arc<Mutex<Vec<Level>>>,
    bids: Arc<Mutex<Vec<Level>>>,
    tx_pool: Arc<Mutex<Vec::<mpsc::Sender<Result<Summary, Status>>>>>,
) -> Result<(), tonic::transport::Error> {
// pub async fn bitstamp_orderbook_client(conf: &config::Server) -> Result<(), tonic::transport::Error> {
        println!("Bitstamp Collector Started...");
    let url = url::Url::parse(&conf.websockets.bitstamp.as_str()).unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect!");
    println!("Bitstamp WebSocket handshake has been successfully completed.");

    let (mut write, read) = ws_stream.split();
    // let mut new_stream = stream_function();


    // let mut new_guard = new_stream.lock().await;
    // let mut guard = s.lock().await;

    // *guard.write( *new_guard);

    // forward(new_stream);

    // send json to ws to select channel
    let buf = "{\"event\":\"bts:subscribe\",\"data\":{\"channel\":\"order_book_btcusd\"}}";
    write.send(buf.into()).await.unwrap();
    let read_future = {
        read.for_each(|message| async {
            // let data = message.unwrap().into_data();
            let msg = match message.unwrap() {
                Message::Text(s) => { s }
                _ => { panic!() }
            };
            // let msg = message.unwrap();
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
                    // println!("about to try to parse : {}", bid);
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
                    spread: 10.0,
                    bids: (*bids_lk).clone(),
                    asks: (*asks_lk).clone(),
                };
                while tx_pool.lock().await.len() == 0 {
                    // println!("tx_pool is empty...");
                    sleep(Duration::from_millis(100)).await;
                }
                match tx_pool.lock().await[0].send(Result::<Summary, Status>::Ok(item)).await {
                    Ok(_) => {
                    // item (server response) was queued to be send to client
                }
                Err(_item) => {
                    // output_stream was build from rx and both are dropped
                    println!("Error sending aggregated orderbook item.");
                }
            }
        }
            // let bids_vec = vec!( parsed["data"]["bids"] );
            // bids_vec.for_each(|x| bids_lk.push(x.into().clone()));
            // data.for_each()
            // tokio::io::stdout().write_all(&data.iter().map(|&x| x.timestamp).collect::<Vec<_>>()).await.unwrap();
        })
    };
    read_future.await;
    Ok(())
}
