use tonic::{transport::Server, Request, Response, Status};

use orderbook::orderbook_aggregator_server::{OrderbookAggregator, OrderbookAggregatorServer};
use orderbook::{Empty, Level, Summary};

pub mod orderbook {
    tonic::include_proto!("orderbook");
}
#[derive(Debug, Default)]
pub struct OrderbookAggregatorService {}

#[tonic::async_trait]
impl OrderbookAggregator for OrderbookAggregatorService {
    async fn book_summary(
        &self,
        request: Request<Empty>
    ) -> Result<Response<Summary>, Status> {
        println!("goit a request: {:?}", request);

        // let req = request.into_inner();
        let reply = Summary {
            spread: 10.0,
            bids: vec![
                Level {
                    exchange: "binance".to_string(),
                    price: 100.1,
                    amount: 500.1,
                }
            ],
            asks: vec![
                Level {
                    exchange: "binance".to_string(),
                    price: 100.1,
                    amount: 500.1,
                }
            ],
        };

        Ok(Response::new(reply))

    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let orderbook_service = OrderbookAggregatorService::default();
    Server::builder()
        .add_service(OrderbookAggregatorServer::new(orderbook_service))
        .serve(addr)
        .await?;
    Ok(())
}
