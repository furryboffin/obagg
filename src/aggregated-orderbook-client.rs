use orderbook::orderbook_aggregator_client::OrderbookAggregatorClient;
use orderbook::Empty;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = tonic::transport::Channel::from_static("http://[::1]:50051")
    .connect()
    .await?;
    let mut client = OrderbookAggregatorClient::new(channel);
    let request = tonic::Request::new(
        Empty{},
    );
    // now the response is stream
    let mut response = client.book_summary_stream(request).await?.into_inner();
    // listening to stream
    while let Some(res) = response.message().await? {
        println!("NOTE = {:?}", res);
    }
    Ok(())
}
