use futures_util::{future, pin_mut, StreamExt, SinkExt};
use crate::config;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async};

pub async fn binance_orderbook_client(conf: &config::Server) -> Result<(), tonic::transport::Error> {
    // Binance requires that the ticker and params be specified in the url. First we must construct
    // the url.
    let base = url::Url::parse(&conf.websockets.binance.as_str()).unwrap();
    let channel = format!("/ws/{}@depth@1000ms",&conf.ticker);
    let url = base
        .join(channel.as_str())
        .unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect!");
    println!("Binance WebSocket handshake has been successfully completed.");

    let (_, read) = ws_stream.split();

    let read_future = {
        read.for_each(|message| async {
            let data = message.unwrap().into_data();
            tokio::io::stdout().write_all(&data).await.unwrap();
        })
    };
    read_future.await;
    Ok(())
}
