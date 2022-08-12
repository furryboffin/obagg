use orderbook::orderbook_aggregator_client::OrderbookAggregatorClient;
use orderbook::Empty;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = OrderbookAggregatorClient::connect("http://[::1]:50051").await?;

    let request = tonic::Request::new(
        Empty{}
    );

    let response = client.book_summary(request).await?;

    println!("RESPONSE={:?}", response);
    Ok(())
}
