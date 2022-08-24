
use std::collections::HashMap;
use std::{error::Error, sync::Arc};
use tokio::{sync::{mpsc, Mutex}, time::{sleep, Duration}};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::Status;
use uuid::Uuid;
use std::sync::mpsc::channel;
use std::thread;

use crate::{ config, orderbook::{Level, Summary} };

pub async fn aggregate_orderbooks(
    conf: &config::Server,
    rx: &mut mpsc::Receiver<Result<Summary, Status>>,
    tx_pool: Arc<Mutex<HashMap<Uuid, mpsc::Sender<Result<Summary, Status>>>>>,
) -> Result<(), Box<dyn Error>> {
    while let Some(msg) = rx.recv().await {
        if let Ok(summary) = msg {
            // wait until we have clients connected.
            while tx_pool.lock().await.len() == 0 {
                sleep(Duration::from_millis(100)).await;
            }

            let tx_pool_locked = tx_pool.lock().await;
            let mut tx_pool_iter = tx_pool_locked.iter();
            let mut futures = vec![];

            while let Some((_, tx)) = tx_pool_iter.next() {
                futures.push(tx.send(Ok(summary.clone())));
            }
            let mut i = 0;
            for r in futures::future::join_all(futures).await {
                if let Err(_item) = r {
                    println!("Error sending aggregated orderbook item : {:?}", i);
                }
                i += 1;
            }
        }
    }
    Ok(())
}
