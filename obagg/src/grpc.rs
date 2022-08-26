use log::info;
use std::{
    collections::HashMap,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::{
    orderbook,
    orderbook::{Empty, Summary},
};

type OrderbookAggregatorResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<Summary, Status>> + Send>>;
type ProducerPool = Arc<Mutex<HashMap<Uuid, mpsc::Sender<Result<Summary, Status>>>>>;

#[derive(Debug)]
pub struct OrderbookAggregatorServer {
    pub tx_pool: ProducerPool,
}

struct DropReceiver<T> {
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
        info!("New gRPC client connected from: {:?}", req.remote_addr());
        let (tx, rx) = mpsc::channel(1024);
        let (oneshot_tx, mut oneshot_rx) = mpsc::unbounded_channel::<usize>();
        let tx_pool_pop = self.tx_pool.clone();
        let id = Uuid::new_v4();

        tokio::spawn(async move {
            info!(
                "Spawned drop handler thread for gRPC producer pool id : {}",
                &id
            );
            while let Some(_) = oneshot_rx.recv().await {
                let mut tx_pool = tx_pool_pop.lock().await;
                info!(
                    "gRPC client connection closed, remove producer pool entry: {}",
                    &id
                );
                tx_pool.remove(&id);
            }
        });

        {
            let mut tx_pool = self.tx_pool.lock().await;
            info!("Add a new gRPC producer pool entry with id {}", &id);
            tx_pool.insert(id, tx.clone());
        }

        let output_stream = DropReceiver {
            chan: oneshot_tx,
            inner: rx,
        };

        let res = Response::new(Box::pin(output_stream) as Self::BookSummaryStreamStream);
        Ok(res)
    }
}
