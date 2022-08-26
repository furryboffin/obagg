use futures::{SinkExt, StreamExt};
use log::{error, info};
use serde_json;
use std::error::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::Status;

use crate::{
    config,
    definitions::{Orderbook, Orderbooks},
    utils,
};

pub async fn consume_orderbooks(
    conf: &config::Server,
    tx: mpsc::Sender<Result<Orderbooks, Status>>,
) -> Result<(), Box<dyn Error>> {
    info!("Bitstamp Collector Started, attempting to connect to websocket server...");
    let url = url::Url::parse(&conf.exchanges.bitstamp.websocket.as_str())?;
    let (ws_stream, _) = connect_async(url).await?;
    info!("Bitstamp WebSocket handshake has been successfully completed.");

    let (mut write, read) = ws_stream.split();

    // send json to ws to select channel
    let buf = format!(
        "{{\"event\":\"bts:subscribe\",\"data\":{{\"channel\":\"order_book_{}\"}}}}",
        conf.ticker
    );
    write.send(buf.into()).await?;

    let read_future = {
        read.for_each(|message| async {
            let mut orderbook = Orderbooks::Bitstamp(Orderbook::new());
            // JRF TODO, we must handle these exceptions where data was not a message.
            // currently the server panics! Could this be caused by running two different binaries?
            let msg = match message.expect("Data was not message!") {
                Message::Text(s) => s,
                _ => {
                    error!("Websocket message was not a string!");
                    return;
                }
            };

            let parsed: serde_json::Value =
                serde_json::from_str(&msg).expect("Can't parse to JSON");
            let mut skip = true;

            if let Some(bids_in) = parsed["data"]["bids"].as_array() {
                skip = false;
                let mut bids_iter = bids_in.into_iter().take(conf.depth);
                while let Some(bid) = bids_iter.next() {
                    orderbook.bids().insert(
                        utils::key_from_value(bid),
                        utils::level_from_value(bid, "bitstamp"),
                    );
                }
            }
            if let Some(asks_in) = parsed["data"]["asks"].as_array() {
                skip = false;
                let mut asks_iter = asks_in.into_iter().take(conf.depth);
                while let Some(ask) = asks_iter.next() {
                    orderbook.asks().insert(
                        utils::key_from_value(ask),
                        utils::level_from_value(ask, "bitstamp"),
                    );
                }
            }

            if !skip {
                if let Err(_item) = tx.send(Result::<Orderbooks, Status>::Ok(orderbook)).await {
                    println!("Error sending bitstamp orderbook item.");
                };
            }
        })
    };
    read_future.await;
    Ok(())
}
