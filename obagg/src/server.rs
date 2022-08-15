use futures::Stream;
use std::{net::ToSocketAddrs, pin::Pin, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{transport::Server, Request, Response, Status};

use orderbook::{Empty, Level, Summary};

use crate::config;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

type OrderbookAggregatorResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<Summary, Status>> + Send>>;

#[derive(Debug)]
pub struct OrderbookAggregatorServer {}

#[tonic::async_trait]
impl orderbook::orderbook_aggregator_server::OrderbookAggregator for OrderbookAggregatorServer {
    async fn book_summary(&self, _: Request<Empty>) -> OrderbookAggregatorResult<Summary> {
        Err(Status::unimplemented("not implemented"))
    }

    type BookSummaryStreamStream = ResponseStream;

    async fn book_summary_stream(
        &self,
        req: Request<Empty>,
    ) -> OrderbookAggregatorResult<Self::BookSummaryStreamStream> {
        println!("OrderbookAggregatorServer::server_streaming_orderbook_aggregator");
        println!("\tclient connected from: {:?}", req.remote_addr());
        // creating infinite stream with requested message
        let repeat = std::iter::repeat(Summary {
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
        });
        let mut stream = Box::pin(tokio_stream::iter(repeat).throttle(Duration::from_millis(200)));

        // spawn and channel are required if you want handle "disconnect" functionality
        // the `out_stream` will not be polled after client disconnect
        let (tx, rx) = mpsc::channel(128);
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                match tx.send(Result::<_, Status>::Ok(item)).await {
                    Ok(_) => {
                        // item (server response) was queued to be send to client
                    }
                    Err(_item) => {
                        // output_stream was build from rx and both are dropped
                        break;
                    }
                }
            }
            println!("\tclient disconnected");
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::BookSummaryStreamStream
        ))
    }
}

pub async fn server(conf: config::Server) {
    let server = OrderbookAggregatorServer {};
    println!("Starting Server... Bind Address: {:?}", &conf.bind_address);
    Server::builder()
        .add_service(orderbook::orderbook_aggregator_server::OrderbookAggregatorServer::new(server))
        // .serve(conf.bind_address)
        .serve("[::1]:50051".to_socket_addrs().unwrap().next().unwrap())
        .await
        .unwrap();
}
