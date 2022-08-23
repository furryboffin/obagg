use futures::Future;
use futures::future::*;
use std::error::Error;
use std::{pin::Pin, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::{Stream, wrappers::ReceiverStream};
use tonic::{transport::Server, Request, Response, Status};

use crate::{
    config,
    // error,
    binance_orderbook_client::binance_orderbook_client,
    bitstamp_orderbook_client::bitstamp_orderbook_client,
    orderbook,
    orderbook::{Empty, Level, Summary},
};

type OrderbookAggregatorResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<Summary, Status>> + Send>>;
type ProducerPool = Arc<Mutex<Vec::<mpsc::Sender<Result<Summary, Status>>>>>;

#[derive(Debug)]
pub struct OrderbookAggregatorServer {
    pub tx_pool: ProducerPool,
}

#[tonic::async_trait]
impl orderbook::orderbook_aggregator_server::OrderbookAggregator for OrderbookAggregatorServer {
    type BookSummaryStreamStream = ResponseStream;
    async fn book_summary_stream(
        &self,
        req: Request<Empty>,
    ) -> OrderbookAggregatorResult<Self::BookSummaryStreamStream> {
        println!("OrderbookAggregatorServer::server_streaming_orderbook_aggregator");
        println!("\tclient connected from: {:?}", req.remote_addr());
        let (tx, rx) = mpsc::channel(128);
        {
            let mut tx_pool = self.tx_pool.lock().await;
            tx_pool.push(tx.clone());
        }
        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::BookSummaryStreamStream
        ))
    }
}

// This server function first launches the gRPC stream server to serve the aggregated orderbook
// followed by launching websocket clients for each exchange.
pub async fn server(conf: config::Server) -> Result<(), Box<dyn Error>> {
    let tx_pool = Vec::<mpsc::Sender<Result<Summary, Status>>>::with_capacity(1);
    let tx_pool = Arc::new(Mutex::new(tx_pool));
    let aggregated_bids = Arc::new(Mutex::new(Vec::<Level>::with_capacity(conf.depth.into())));
    let aggregated_asks = Arc::new(Mutex::new(Vec::<Level>::with_capacity(conf.depth.into())));
    let server = OrderbookAggregatorServer {tx_pool: tx_pool.clone()};
    let mut futures = vec![];

    futures.push(
        Box::pin(
            Server::builder()
                .add_service(orderbook::orderbook_aggregator_server::OrderbookAggregatorServer::new(server))
                .serve(conf.bind_address)
                .map_err(|e| e.into())
        ) as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>
    );
    println!("Started gRPC Server... Bind Address: {:?}", &conf.bind_address);

    // JRF TODO move the two orderbook clients into spawned threads to help speed up processing if required.
    // futures.push( Box::pin( binance_orderbook_client(&conf) ) as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>> );
    futures.push( Box::pin( bitstamp_orderbook_client(&conf, aggregated_asks, aggregated_bids, tx_pool.clone()) ) as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>> );
    for r in futures::future::join_all(futures).await {
        if let Err(e) = r {
            return Err(e);
        }
    }
    Ok(())
}
