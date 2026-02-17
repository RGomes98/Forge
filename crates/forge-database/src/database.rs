use std::sync::{Arc, atomic};
use std::thread;

use super::DatabaseError;
use super::RowSet;
use super::db_connection::DbConnection;
use super::sql_args::SqlArg;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{mpsc, oneshot};

type DbResult = Result<RowSet, DatabaseError>;
type DbReplySender = oneshot::Sender<DbResult>;
type DbReplyReceiver = oneshot::Receiver<DbResult>;
type DbSender = mpsc::Sender<DbCommand>;
type DbReceiver = mpsc::Receiver<DbCommand>;

const BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
pub struct DatabaseOptions {
    pub url: String,
    pub threads: usize,
    pub inflight_per_conn: usize,
}

#[derive(Debug)]
pub enum DbCommand {
    Execute {
        query: Arc<str>,
        args: Vec<SqlArg>,
        reply: DbReplySender,
    },
}

#[derive(Debug)]
pub struct Database {
    counter: Arc<atomic::AtomicUsize>,
    senders: Arc<Vec<mpsc::Sender<DbCommand>>>,
}

impl Database {
    pub fn new(options: DatabaseOptions) -> Result<Self, DatabaseError> {
        assert!(options.threads > 0);
        assert!(options.inflight_per_conn > 0);

        let runtime: Runtime = Builder::new_multi_thread()
            .worker_threads(options.threads)
            .enable_all()
            .build()?;

        let (senders, receivers): (Vec<DbSender>, Vec<DbReceiver>) = (0..options.threads)
            .map(|_| mpsc::channel::<DbCommand>(BUFFER_SIZE))
            .unzip();

        let inflight: usize = options.inflight_per_conn;

        thread::spawn(move || {
            runtime.block_on(async move {
                for (idx, receiver) in receivers.into_iter().enumerate() {
                    let url: String = options.url.clone();

                    tokio::spawn(async move {
                        match DbConnection::new(url, inflight, receiver).await {
                            Err(e) => eprintln!("DbConnection #{idx} failed to start: {e:#?}"),
                            Ok(mut conn) => conn.process_queue().await,
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

    pub async fn query(&self, query: impl Into<Arc<str>>, args: Vec<SqlArg>) -> DbResult {
        let (reply, receiver): (DbReplySender, DbReplyReceiver) = oneshot::channel();
        let idx: usize = self.counter.fetch_add(1, atomic::Ordering::Relaxed) % self.senders.len();
        let query: Arc<str> = query.into();

        self.senders[idx]
            .send(DbCommand::Execute { query, args, reply })
            .await?;

        receiver.await?
    }
}
