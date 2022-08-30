use futures_util::StreamExt;
use log::{debug, error, info};
use serde_json;
use std::{error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::Status;

use crate::{
    config,
    definitions::{BinanceOrderbookMessage, BinanceOrderbookUpdateMessage, Orderbook, Orderbooks},
    utils,
};

const EXCHANGE: &str = "binance";

// For depths 20 and under we employ the reduced orderbook stream.
pub async fn consume_reduced_orderbooks(
    conf: &config::Server,
    tx: &mpsc::Sender<Result<Orderbooks, Status>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("Binance Collector Started, attempting to connect to websocket server...");
    let base = url::Url::parse(&conf.exchanges.binance.websocket.as_str())?;
    let channel = format!("/ws/{}@depth{}@100ms", &conf.ticker, &conf.depth); //<symbol>@depth<levels>@100ms
    let url = base.join(channel.as_str())?;
    let (ws_stream, _) = connect_async(url).await?;
    info!("Binance WebSocket handshake has been successfully completed.");

    let (_, read) = ws_stream.split();

    let read_future = {
        read.for_each(|message| async {
            let mut orderbook = Orderbooks::Binance(Orderbook::new());
            match message {
                Ok(message) => {
                    // if let Ok(message) = message {
                    let msg = match message {
                        Message::Text(s) => s,
                        _ => {
                            debug!("Websocket message was not a string!");
                            return;
                        }
                    };
                    match serde_json::from_str::<BinanceOrderbookMessage>(&msg) {
                        Ok(orderbook_message) => {
                            for bid in orderbook_message.bids {
                                orderbook
                                    .bids()
                                    .insert(bid.get_price(), bid.get_level(EXCHANGE));
                            }
                            for ask in orderbook_message.asks {
                                orderbook
                                    .asks()
                                    .insert(ask.get_price(), ask.get_level(EXCHANGE));
                            }
                            if let Err(_item) =
                                tx.send(Result::<Orderbooks, Status>::Ok(orderbook)).await
                            {
                                error!("Error sending binance orderbook item.");
                            };
                        }
                        Err(err) => {
                            debug!("Message is not an Orderbook message. {}: msg {}", err, msg);
                        }
                    }
                }
                Err(err) => {
                    error!("Data was not message!");
                    error!("{}", err);
                }
            }
        })
    };
    read_future.await;
    error!("Websocket failed and closed!");
    Ok(())
}

// For depths over 20 we must employ the full orderbook websocket channel.
// In this case we open a websocket connection and process the update messages into a locally stored
// orderbook. The following set of rules are applied:
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
    tx: &mpsc::Sender<Result<Orderbooks, Status>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Binance requires that the ticker and params be specified in the url. First we must construct
    // the url.
    let orderbook_arc = Arc::new(Mutex::new(Orderbook::new()));
    let ws_base = url::Url::parse(&conf.exchanges.binance.websocket.as_str())?;
    let ws_channel = format!("/ws/{}@depth@100ms", &conf.ticker);
    let ws_url = ws_base.join(ws_channel.as_str())?;

    info!("Binance Collector Started, attempting to connect to websocket server...");
    let (ws_stream, _) = connect_async(ws_url).await?;
    info!("Binance WebSocket handshake has been successfully completed.");
    let (_, read) = ws_stream.split();

    // get the snapshot
    let last_update_id_arc = Arc::new(Mutex::new(
        get_snapshot(conf, &mut *orderbook_arc.lock().await).await?,
    ));
    let is_first = Arc::new(Mutex::new(true));
    let prev_u = Arc::new(Mutex::new(0));

    // now that we have the order_book snapshot, we can process updates
    let read_future = {
        read.for_each(|message| async {
            let mut is_first_lk = is_first.lock().await;
            let mut prev_u_lk = prev_u.lock().await;
            let mut last_update_id = last_update_id_arc.lock().await;
            let mut orderbook = orderbook_arc.lock().await;
            match message {
                Ok(message) => {
                    // if let Ok(message) = message {
                    let msg = match message {
                        Message::Text(s) => s,
                        _ => {
                            debug!("Websocket message was not a string!");
                            match get_snapshot(conf, &mut orderbook).await {
                                Ok(lu) => *last_update_id = lu,
                                Err(e) => {
                                    error!("Failed to get snapshot. {}", e);
                                    return;
                                }
                            }
                            *is_first_lk = true;
                            return;
                        }
                    };

                    match serde_json::from_str::<BinanceOrderbookUpdateMessage>(&msg) {
                        Ok(orderbook_message) => {
                            if orderbook_message.last_update_id <= *last_update_id {
                                return;
                            }
                            if !*is_first_lk && *prev_u_lk + 1 != orderbook_message.first_update_id
                            {
                                error!("Update out of sequence.");
                                match get_snapshot(conf, &mut orderbook).await {
                                    Ok(lu) => *last_update_id = lu,
                                    Err(e) => {
                                        error!("Failed to get snapshot. {}", e);
                                        return;
                                    }
                                }
                                *is_first_lk = true;
                                return;
                            }

                            if *is_first_lk
                                && orderbook_message.first_update_id <= *last_update_id + 1
                                && orderbook_message.last_update_id >= *last_update_id + 1
                            {
                                *is_first_lk = false;
                            } else if *is_first_lk {
                                error!("Update out of sequence.");
                                match get_snapshot(conf, &mut orderbook).await {
                                    Ok(lu) => *last_update_id = lu,
                                    Err(e) => {
                                        error!("Failed to get snapshot. {}", e);
                                        return;
                                    }
                                }
                                *is_first_lk = true;
                                return;
                            }

                            *prev_u_lk = orderbook_message.last_update_id;

                            let bids_in = orderbook_message.bids;
                            utils::handle_update_message(
                                &bids_in,
                                &mut orderbook,
                                conf.depth,
                                true,
                                EXCHANGE,
                            );

                            let asks_in = orderbook_message.asks;
                            utils::handle_update_message(
                                &asks_in,
                                &mut orderbook,
                                conf.depth,
                                false,
                                EXCHANGE,
                            );

                            // reduce the depth of the orderbook if required
                            let orderbook_reduced = orderbook.reduce(conf.depth);

                            if let Err(_item) = tx
                                .send(Result::<Orderbooks, Status>::Ok(Orderbooks::Binance(
                                    orderbook_reduced,
                                )))
                                .await
                            {
                                error!("Error sending binance orderbook item.");
                            };
                        }
                        Err(err) => {
                            debug!("Message is not an Orderbook message. {}: msg {}", err, msg);
                        }
                    }
                }
                Err(err) => {
                    error!("Data was not message!");
                    error!("{}", err);
                }
            }
        })
    };
    read_future.await;
    error!("Websocket failed and closed!");
    Ok(())
}

// Get a snapshot of the orderbook from the binance API server. This async function returns a promise
// that resolves to a Result<lastUpdateId> returned with the orderbook data. The bids and asks are stored
// in the orderbook reference object that is passed into the function call.
async fn get_snapshot(
    conf: &config::Server,
    orderbook: &mut Orderbook,
) -> Result<u64, Box<dyn Error + Send + Sync>> {
    let api_base = url::Url::parse(&conf.exchanges.binance.api.as_str())?;
    let api_channel = format!(
        "/api/v3/depth?symbol={}&limit={}",
        &conf.ticker.to_uppercase(),
        100
    );
    let api_url = api_base.join(api_channel.as_str())?;
    let snapshot = reqwest::get(api_url).await?.text().await?;
    let orderbook_message = serde_json::from_str::<BinanceOrderbookMessage>(&snapshot)?;
    orderbook.bids.clear();
    orderbook.asks.clear();
    for bid in orderbook_message.bids {
        orderbook
            .bids
            .insert(bid.get_price(), bid.get_level(EXCHANGE));
    }
    for ask in orderbook_message.asks {
        orderbook
            .asks
            .insert(ask.get_price(), ask.get_level(EXCHANGE));
    }
    Ok(orderbook_message.last_update_id)
}
