use futures::{future::*, Future};
use log::{info, warn};
use std::{collections::HashMap, error::Error, pin::Pin, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tonic::{transport::Server, Status};

use crate::{
    aggregator, binance, bitstamp, config, definitions::Orderbooks,
    grpc::OrderbookAggregatorServer, orderbook,
};

// This server function first launches the gRPC stream server to serve the aggregated orderbook
// followed by launching websocket clients for each exchange.
pub async fn server(conf: config::Server) -> Result<(), Box<dyn Error + Send + Sync>> {
    let tx_pool = HashMap::new();
    let tx_pool = Arc::new(Mutex::new(tx_pool));
    let (binance_orderbook_ws_tx, mut aggregator_rx) =
        mpsc::channel::<Result<Orderbooks, Status>>(1024); // or bounded
    let bitstamp_orderbook_ws_tx = binance_orderbook_ws_tx.clone();
    let server = OrderbookAggregatorServer {
        tx_pool: tx_pool.clone(),
    };
    let binance_conf = conf.clone();
    let bitstamp_conf = conf.clone();
    let aggregator_conf = conf.clone();

    // launch the server in the main thread.
    let server_future = Box::pin(
        Server::builder()
            .add_service(
                orderbook::orderbook_aggregator_server::OrderbookAggregatorServer::new(server),
            )
            .serve(conf.bind_address)
            .map_err(|e| e.into()),
    )
        as Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>>>>;
    info!(
        "Started gRPC Server... Bind Address: {:?}",
        &conf.bind_address
    );

    // launch the binance orderbook consumer in a thread
    if conf.exchanges.binance.enable {
        tokio::spawn(async move {
            info!("Spawned binance websocket consumer.");
            if binance_conf.depth <= 20 {
                while let Ok(_) = binance::consume_reduced_orderbooks(
                    &binance_conf,
                    binance_orderbook_ws_tx.clone(),
                )
                .await
                {
                    warn!("Relaunching binance websocket consumer.");
                }
            } else {
                while let Ok(_) =
                    binance::consume_orderbooks(&binance_conf, binance_orderbook_ws_tx.clone())
                        .await
                {
                    warn!("Relaunching binance websocket consumer.");
                }
            }
        });
    }

    // launch the bitstamp orderbook consumer in a thread
    if conf.exchanges.bitstamp.enable {
        tokio::spawn(async move {
            while let Ok(_) =
                bitstamp::consume_orderbooks(&bitstamp_conf, &bitstamp_orderbook_ws_tx).await
            {
                warn!("Relaunching bitstamp websocket consumer.");
            }
        });
    }

    // launch the orderbook aggregator in a thread
    tokio::spawn(async move {
        while let Ok(_) =
            aggregator::aggregate_orderbooks(&aggregator_conf, &mut aggregator_rx, tx_pool.clone())
                .await
        {
            warn!("Relaunching bitstamp websocket consumer.");
        }
    });

    server_future.await?;
    Ok(())
}
