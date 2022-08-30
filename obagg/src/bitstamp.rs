use futures::{SinkExt, StreamExt};
use log::{debug, error, info};
use serde_json;
use std::error::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::Status;

use crate::{
    config,
    definitions::{BitstampOrderbookMessage, Orderbook, Orderbooks},
};

const EXCHANGE: &str = "bitstamp";

pub async fn consume_orderbooks(
    conf: &config::Server,
    tx: &mpsc::Sender<Result<Orderbooks, Status>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
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
            let mut orderbook = Orderbook::new();
            match message {
                Ok(message) => {
                    let msg = match message {
                        Message::Text(s) => s,
                        _ => {
                            // JRF TODO do I need to reconnect when this happens?
                            debug!("Websocket message was not a string!");
                            return;
                        }
                    };
                    match serde_json::from_str::<BitstampOrderbookMessage>(&msg) {
                        Ok(orderbook_message) => {
                            for bid in orderbook_message.data.bids {
                                orderbook
                                    .bids
                                    .insert(bid.get_price(), bid.get_level(EXCHANGE));
                            }
                            for ask in orderbook_message.data.asks {
                                orderbook
                                    .asks
                                    .insert(ask.get_price(), ask.get_level(EXCHANGE));
                            }
                            if let Err(_item) = tx
                                .send(Result::<Orderbooks, Status>::Ok(Orderbooks::Bitstamp(
                                    orderbook.reduce(conf.depth),
                                )))
                                .await
                            {
                                error!("Error sending bitstamp orderbook item.");
                            };
                        }
                        Err(err) => {
                            // JRF TODO do I need to reconnect when this happens?
                            debug!("Message is not an Orderbook message. {}: msg {}", err, msg);
                        }
                    }
                }
                Err(err) => {
                    error!("Data was not a message!");
                    error!("{}", err);
                }
            }
        })
    };
    read_future.await;
    error!("Websocket failed and closed!");
    Ok(())
}
