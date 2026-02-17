use std::sync::{Arc, atomic};
use std::thread;

use super::DatabaseError;
use super::RowSet;
use super::sql_args::SqlArg;
use super::worker::Worker;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{mpsc, oneshot};

type PgActorPayload = Result<RowSet, DatabaseError>;
type PgReplySender = oneshot::Sender<PgActorPayload>;
type PgReplyReceiver = oneshot::Receiver<PgActorPayload>;
type PgSender = mpsc::Sender<ActorMessage>;
type PgReceiver = mpsc::Receiver<ActorMessage>;

const BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
pub struct PgOptions {
    pub pool_size: usize,
    pub database_url: String,
    pub inflight_per_conn: usize,
}

#[derive(Debug)]
pub enum ActorMessage {
    Execute {
        query: Arc<str>,
        args: Vec<SqlArg>,
        sender: PgReplySender,
    },
}

#[derive(Debug)]
pub struct PgActor {
    counter: Arc<atomic::AtomicUsize>,
    senders: Arc<Vec<mpsc::Sender<ActorMessage>>>,
}

impl PgActor {
    pub fn new(options: PgOptions) -> Result<Self, DatabaseError> {
        assert!(options.pool_size > 0);
        assert!(options.inflight_per_conn > 0);

        let runtime: Runtime = Builder::new_multi_thread()
            .worker_threads(options.pool_size)
            .enable_all()
            .build()?;

        let (senders, receivers): (Vec<PgSender>, Vec<PgReceiver>) = (0..options.pool_size)
            .map(|_| mpsc::channel::<ActorMessage>(BUFFER_SIZE))
            .unzip();

        let inflight: usize = options.inflight_per_conn;

        thread::spawn(move || {
            runtime.block_on(async move {
                for (idx, receiver) in receivers.into_iter().enumerate() {
                    let database_url: String = options.database_url.clone();

                    tokio::spawn(async move {
                        match Worker::new(database_url, inflight, receiver).await {
                            Err(e) => eprintln!("DB worker {idx} failed to initialize: {e}"),
                            Ok(mut worker) => worker.dispatch().await,
                        }
                    });
                }

                std::future::pending::<()>().await;
            });
        });

        Ok(Self {
            senders: Arc::new(senders),
            counter: Arc::new(atomic::AtomicUsize::new(0)),
        })
    }

    pub async fn query(&self, query: impl Into<Arc<str>>, args: Vec<SqlArg>) -> PgActorPayload {
        let (sender, receiver): (PgReplySender, PgReplyReceiver) = oneshot::channel();
        let idx: usize = self.counter.fetch_add(1, atomic::Ordering::Relaxed) % self.senders.len();
        let query: Arc<str> = query.into();

        self.senders[idx]
            .send(ActorMessage::Execute { query, args, sender })
            .await?;

        receiver.await?
    }
}
