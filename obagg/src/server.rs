use futures::Future;
use async_stream::stream;
use std::sync::Arc;
use tokio::{sync::Mutex};
use tokio::time::{sleep, Duration};

// use futures_util::{future, pin_mut};
// use tokio_tungstenite::tungstenite::stream;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio::pin;
use tokio_stream::{Stream, wrappers::ReceiverStream, StreamExt};
use tonic::{transport::Server, Request, Response, Status};

// use orderbook::{Empty, Level, Summary};
// use orderbook::{Empty, Level, Summary};
// pub mod orderbook {
//     tonic::include_proto!("orderbook");
// }
use crate::{
    config,
    binance_orderbook_client::binance_orderbook_client,
    bitstamp_orderbook_client::bitstamp_orderbook_client,
    orderbook,
    orderbook::{Empty, Level, Summary},
};

// pub mod orderbook {
//     tonic::include_proto!("orderbook");
// }

type OrderbookAggregatorResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<Summary, Status>> + Send>>;
type ResponseStreamSync = Arc<Mutex<Pin<Box<dyn Stream<Item = Summary> + Send + Sync + 'static>>>>;
// type ResponseStreamSync = Pin<Box<Arc<Mutex<dyn Stream<Item = Summary> + Send + Sync>>>>;
// type ResponseStreamSync = Pin<Box<std::sync::Arc<std::sync::Mutex<Throttle<tokio_stream::Iter<std::iter::Repeat<orderbook::Summary>>>>>>>;
// type ResponseStreamSync = Pin<Box<std::sync::Arc<std::sync::Mutex<Throttle<>>>>>;
// type BookSummaryStreamStream = ResponseStream;
type BookSummaryStreamStreamSync = ResponseStreamSync;

// #[derive(Debug)]
pub struct OrderbookAggregatorServer {
    // pub stream: BookSummaryStreamStreamSync,
    // pub rx: Arc<Mutex<mpsc::Receiver<Result<Summary, Status>>>>,
    // pub tx_pool: Arc<Mutex<Vec::<Arc<Mutex<mpsc::Sender<Result<Summary, Status>>>>>>>,
    pub tx_pool: Arc<Mutex<Vec::<mpsc::Sender<Result<Summary, Status>>>>>,
    // pub tx_pool: Vec::<Arc<Mutex<mpsc::Sender<Result<Summary, Status>>>>>,
    // pub rx: Arc<mpsc::Receiver<Result<Summary, Status>>>,
}

// fn lock<T>(stream:Arc<Mutex<T>> ) -> MutexGuard<T>{
//     stream.lock().unwrap()
// }



// impl TxPool {
//     pub fn push(&mut self, tx: Arc<mpsc::Sender<Result<Summary, Status>>>) {
//         self.tx_pool.push(tx);
//     }
//     pub fn tx(&self) -> &'static mut Arc<mpsc::Sender<Result<Summary, Status>>> {
//         &mut self.tx_pool[0]
//     }
// }
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
        // spawn and channel are required if you want handle "disconnect" functionality
        // the `out_stream` will not be polled after client disconnect
        let (tx, rx) = mpsc::channel(128);
        // let tx = Arc::new(Mutex::new(tx));
        {
            self.tx_pool.lock().await.push(tx.clone());
            // self.tx_pool.push(tx.clone());

        // let s = self.stream.clone();

            // tokio::spawn(async move {
            //     // if let Ok(mut x) = stream.lock() {
            //         // println!("Thread 1 acquired lock");
            //         while let Some(item) = stream.lock().await.next().await {
            //             match tx.lock().await.send(Result::<Summary, Status>::Ok(item)).await {
            //                 Ok(_) => {
            //                     // item (server response) was queued to be send to client
            //                 }
            //                 Err(_item) => {
            //                     // output_stream was build from rx and both are dropped
            //                     break;
            //                 }
            //             }
            //         }
            //     // }
            //     println!("\tclient disconnected");
            // });
        }
        // let rx = self.rx.clone();
        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::BookSummaryStreamStream
        ))
    }
}

// This server function first launches the gRPC stream server to serve the aggregated orderbook
// followed by launching websocket clients for each exchange.
pub async fn server(conf: config::Server) {
    let tx_pool = Vec::<mpsc::Sender<Result<Summary, Status>>>::with_capacity(1);
    let tx_pool = Arc::new(Mutex::new(tx_pool));
    let aggregated_bids = Arc::new(Mutex::new(Vec::<Level>::with_capacity(conf.depth.into())));
    let aggregated_asks = Arc::new(Mutex::new(Vec::<Level>::with_capacity(conf.depth.into())));

    let stream = stream_function();
    let server = OrderbookAggregatorServer {tx_pool: tx_pool.clone()};

    println!("Starting gRPC Server... Bind Address: {:?}", &conf.bind_address);
    let mut futures = vec![];
    futures.push(
        Box::pin(
            Server::builder()
                .add_service(orderbook::orderbook_aggregator_server::OrderbookAggregatorServer::new(server))
                .serve(conf.bind_address)
        ) as Pin<Box<dyn Future<Output = Result<(), tonic::transport::Error>>>>
    );
    println!("Started gRPC Server... Bind Address: {:?}", &conf.bind_address);
    // JRF TODO move the two orderbook clients into swaned threads to help speed up processing if required.
    // futures.push( Box::pin( bitstamp_orderbook_client(&conf, tx_pool.clone()) ) as Pin<Box<dyn Future<Output = Result<(), tonic::transport::Error>>>> );
    futures.push( Box::pin( bitstamp_orderbook_client(&conf, aggregated_asks, aggregated_bids, tx_pool.clone()) ) as Pin<Box<dyn Future<Output = Result<(), tonic::transport::Error>>>> );
    println!("created bitstamp orderbook client in server thread.");
    // futures.push( Box::pin( binance_orderbook_client(&conf) ) as Pin<Box<dyn Future<Output = Result<(), tonic::transport::Error>>>> );
    tokio::spawn(async move {
        // When we first start up we need to wait until the tx_pool has been populated before we can
        // use the tx producer.
        while tx_pool.lock().await.len() == 0 {
            // println!("tx_pool is empty...");
            sleep(Duration::from_millis(100)).await;
        }
        while let Some(item) = stream.lock().await.next().await {
            match tx_pool.lock().await[0].send(Result::<Summary, Status>::Ok(item)).await {
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
    futures::future::join_all(futures).await;
}


pub fn stream_function() -> Arc<Mutex<Pin<Box<dyn Stream<Item = Summary> + Send + Sync + 'static>>>> {
    // creating infinite stream with requested message
    Arc::new(Mutex::new(Box::pin(stream! {
        let entries = vec!(
            Summary {
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
            },
            Summary {
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
            }
        );

        for entry in entries {
            yield entry;
        }
    })))
}

// let repeat = std::iter::repeat(Summary {
//     spread: 10.0,
//     bids: vec![
//         Level {
//             exchange: "binance".to_string(),
//             price: 100.1,
//             amount: 500.1,
//         }
//     ],
//     asks: vec![
//         Level {
//             exchange: "binance".to_string(),
//             price: 100.1,
//             amount, 500.1,
//         }
//     ]
// })
