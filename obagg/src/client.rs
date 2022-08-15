use orderbook::orderbook_aggregator_client::OrderbookAggregatorClient;
use orderbook::Empty;

use crate::config;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
pub async fn client(conf: config::Server) {
        let channel = tonic::transport::Channel::from_static("http://[::1]:50051")
    .connect()
    .await.unwrap();
    let mut client = OrderbookAggregatorClient::new(channel);
    let request = tonic::Request::new(
        Empty{},
    );
    // now the response is stream
    let mut response = client.book_summary_stream(request).await.unwrap().into_inner();
    // listening to stream
    while let Some(res) = response.message().await.unwrap() {
        println!("NOTE = {:?}", res);
    }
}
