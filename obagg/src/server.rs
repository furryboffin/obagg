use futures::{future::*, Future};
use log::info;
use std::{collections::HashMap, error::Error, pin::Pin, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tonic::{transport::Server, Status};

use crate::{
    aggregator, binance, bitstamp, config, definitions::Orderbooks,
    grpc::OrderbookAggregatorServer, orderbook,
};

// This server function first launches the gRPC stream server to serve the aggregated orderbook
// followed by launching websocket clients for each exchange.
pub async fn server(conf: config::Server) -> Result<(), Box<dyn Error>> {
    let tx_pool = HashMap::new();
    let tx_pool = Arc::new(Mutex::new(tx_pool));
    let (orderbook_ws_tx, mut aggregator_rx) = mpsc::channel::<Result<Orderbooks, Status>>(1024); // or bounded
    let server = OrderbookAggregatorServer {
        tx_pool: tx_pool.clone(),
    };
    let mut futures = vec![];

    futures.push(Box::pin(
        Server::builder()
            .add_service(
                orderbook::orderbook_aggregator_server::OrderbookAggregatorServer::new(server),
            )
            .serve(conf.bind_address)
            .map_err(|e| e.into()),
    )
        as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>);
    info!(
        "Started gRPC Server... Bind Address: {:?}",
        &conf.bind_address
    );

    // JRF TODO move the following tasks into spawned threads to help speed up processing if required.
    if conf.exchanges.binance.enable {
        if conf.depth <= 20 {
            futures.push(
                Box::pin(binance::consume_reduced_orderbooks(&conf, orderbook_ws_tx.clone()))
                    as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>,
            );
        } else {
            futures.push(
                Box::pin(binance::consume_orderbooks(&conf, orderbook_ws_tx.clone()))
                    as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>,
            );
        }
    }
    if conf.exchanges.bitstamp.enable {
        futures.push(
            Box::pin(bitstamp::consume_orderbooks(&conf, orderbook_ws_tx))
                as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>,
        );
    }
    futures.push(Box::pin(aggregator::aggregate_orderbooks(
        &conf,
        &mut aggregator_rx,
        tx_pool.clone(),
    ))
        as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>);
    for r in futures::future::join_all(futures).await {
        if let Err(e) = r {
            return Err(e);
        }
    }
    Ok(())
}
