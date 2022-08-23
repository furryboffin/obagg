use http::Uri;
use orderbook::{Empty, orderbook_aggregator_client::OrderbookAggregatorClient};
use std::error::Error;

use crate::config;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

pub async fn client(conf: config::Server) -> Result<(), Box<dyn Error>> {
    let uri = Uri::builder()
        .scheme("http")
        .authority(conf.bind_address.to_string())
        .path_and_query("")
        .build()
        .unwrap();
    println!("Client attempting to connect to : {:?}", uri.clone());
    let channel = tonic::transport::Channel::builder(uri.clone())
        .connect()
        .await.unwrap();
    println!("Client connected to : {:?}", uri);

    let mut client = OrderbookAggregatorClient::new(channel);
    let request = tonic::Request::new(
        Empty{},
    );
    // now the response is a stream
    let mut response = client.book_summary_stream(request).await.unwrap().into_inner();

    // listening to stream
    while let Some(res) = response.message().await? {
        println!("NOTE = {:?}", res);
    }
    Ok(())
    // client.close_summary_stream();
}
