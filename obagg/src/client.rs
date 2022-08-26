use http::Uri;
use log::info;
use orderbook::{orderbook_aggregator_client::OrderbookAggregatorClient, Empty};
use std::error::Error;
use tokio::time::{sleep, Duration};

use crate::config;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

pub async fn client(conf: config::Server) -> Result<(), Box<dyn Error>> {
    let uri = Uri::builder()
        .scheme("http")
        .authority(conf.bind_address.to_string())
        .path_and_query("")
        .build()?;
    let channel;
    loop {
        info!("Client attempting to connect to : {:?}", uri.clone());
        let res = tonic::transport::Channel::builder(uri.clone())
            .connect()
            .await;
        if let Ok(ch) = res {
            channel = ch;
            break;
        }
        sleep(Duration::from_millis(1000)).await;
    }

    info!("Client connected to : {:?}", uri);

    let mut client = OrderbookAggregatorClient::new(channel);
    let request = tonic::Request::new(Empty {});
    // now the response is a stream
    let mut response = client.book_summary_stream(request).await?.into_inner();
    // listening to stream
    while let Some(res) = response.message().await? {
        print!("{esc}c", esc = 27 as char);
        println!("_____________________________________________________________________________\n");
        println!(
            "           {}             SPREAD = {:.8}",
            conf.ticker, res.spread
        );
        println!("_____________________________________________________________________________");
        println!("                                                                             ");
        println!("               Bids                                    Asks                  ");
        println!("_____________________________________________________________________________");
        for it in res.bids.iter().zip(res.asks.iter()) {
            let (bid, ask) = it;
            println!(
                "{}{}  {}{:.6} @ {:5.8}  | {}{:.6} @ {:5.8}   {}",
                bid.exchange,
                if bid.exchange == "binance" {" "} else {""},
                if bid.amount >= 100.0 {
                    ""
                } else if bid.amount >= 10.0 {
                    " "
                } else {
                    "  "
                },
                bid.amount,
                bid.price,
                if ask.amount >= 100.0 {
                    ""
                } else if ask.amount >= 10.0 {
                    " "
                } else {
                    "  "
                },
                ask.amount,
                ask.price,
                ask.exchange,
            );
        }
    }
    Ok(())
    // client.close_summary_stream();
}
