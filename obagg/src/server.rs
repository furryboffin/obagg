use futures::Future;
use futures::future::*;
use log::info;
use std::collections::HashMap;
use std::error::Error;
use std::task::Context;
use std::task::Poll;
use std::{pin::Pin, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

// use tokio_stream::{Stream, wrappers::ReceiverStream};
use tokio_stream::Stream;
use tonic::{transport::Server, Request, Response, Status};

use crate::{
    aggregator,
    config,
    // error,
    binance,
    bitstamp,
    orderbook,
    orderbook::{Empty, Level, Summary},
};

type OrderbookAggregatorResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<Summary, Status>> + Send>>;
type ProducerPool = Arc<Mutex<HashMap<Uuid, mpsc::Sender<Result<Summary, Status>>>>>;

#[derive(Debug)]
pub struct OrderbookAggregatorServer {
    pub tx_pool: ProducerPool,
}

pub struct DropReceiver<T> {
    chan: mpsc::UnboundedSender<usize>,
    inner: mpsc::Receiver<T>,
}

impl<T> Stream for DropReceiver<T> {
    type Item = T;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.inner.poll_recv(cx)
    }
}

impl<T> Drop for DropReceiver<T> {
    fn drop(&mut self) {
        info!("Receiver has been dropped");
        self.chan.send(1).unwrap();
    }
}

#[tonic::async_trait]
impl orderbook::orderbook_aggregator_server::OrderbookAggregator for OrderbookAggregatorServer {
    type BookSummaryStreamStream = ResponseStream;
    async fn book_summary_stream(
        &self,
        req: Request<Empty>,
    ) -> OrderbookAggregatorResult<Self::BookSummaryStreamStream> {
        info!("OrderbookAggregatorServer::server_streaming_orderbook_aggregator");
        info!("\tclient connected from: {:?}", req.remote_addr());
        let (tx, rx) = mpsc::channel(1024);
        let (oneshot_tx, mut oneshot_rx) = mpsc::unbounded_channel::<usize>();
        let tx_pool_pop = self.tx_pool.clone();
        let id = Uuid::new_v4();

        tokio::spawn(async move {
            info!("Spawned drop handler thread...");
            // let future = oneshot_rx.map(|i| {
            while let Some(res) = oneshot_rx.recv().await {
                info!("got: {:?}", res);
                let mut tx_pool = tx_pool_pop.lock().await;
                info!("Remove tx pool entry...");
                // JRF TODO, change this to hashmap
                tx_pool.remove(&id);
            };
        });
        {
            let mut tx_pool = self.tx_pool.lock().await;
            info!("Add a new tx pool entry...");
            tx_pool.insert(id, tx.clone());
        }
        // let output_stream = ReceiverStream::new(rx);
        let output_stream = DropReceiver {
            chan: oneshot_tx,
            inner: rx
        };
        info!("created output_stream...");
        let res = Response::new(
            Box::pin(output_stream) as Self::BookSummaryStreamStream
        );
        info!("created response...");

        Ok(res)
    }
}

// This server function first launches the gRPC stream server to serve the aggregated orderbook
// followed by launching websocket clients for each exchange.
pub async fn server(conf: config::Server) -> Result<(), Box<dyn Error>> {
    // let tx_pool = Vec::<mpsc::Sender<Result<Summary, Status>>>::with_capacity(1);
    let tx_pool = HashMap::new();
    let tx_pool = Arc::new(Mutex::new(tx_pool));
    let (orderbook_ws_tx, mut aggregator_rx) = mpsc::channel::<Result<Summary, Status>>(1024); // or bounded
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
    info!("Started gRPC Server... Bind Address: {:?}", &conf.bind_address);

    // JRF TODO move the two orderbook clients into spawned threads to help speed up processing if required.
    futures.push( Box::pin( binance::consume_orderbooks(&conf,orderbook_ws_tx.clone()) ) as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>> );
    // futures.push( Box::pin( bitstamp::consume_orderbooks(&conf, orderbook_ws_tx) ) as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>> );
    futures.push( Box::pin( aggregator::aggregate_orderbooks(&conf, &mut aggregator_rx, tx_pool.clone()) ) as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>> );
    for r in futures::future::join_all(futures).await {
        if let Err(e) = r {
            return Err(e);
        }
    }
    Ok(())
}
