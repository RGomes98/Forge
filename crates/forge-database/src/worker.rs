use std::sync::Arc;

use super::RowSet;
use super::actor::ActorMessage;
use super::error::DatabaseError;
use super::sql_args::SqlArg;
use forge_utils::LruCache;
use tokio::sync::{Semaphore, mpsc::Receiver};
use tokio_postgres::tls::NoTlsStream;
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, Connection, Error, NoTls, Socket, Statement};

const LRU_CACHE_SIZE: usize = 256;

#[derive(Debug)]
pub struct Worker {
    client: Arc<Client>,
    semaphore: Arc<Semaphore>,
    receiver: Receiver<ActorMessage>,
    cache: LruCache<Arc<str>, Statement>,
}

impl Worker {
    pub async fn new(
        database_url: String,
        inflight_per_conn: usize,
        receiver: Receiver<ActorMessage>,
    ) -> Result<Self, DatabaseError> {
        let (client, connection): (Client, Connection<Socket, NoTlsStream>) =
            tokio_postgres::connect(&database_url, NoTls).await?;

        tokio::spawn(async move {
            connection.await?;
            Ok::<(), Error>(())
        });

        Ok(Self {
            receiver,
            client: Arc::new(client),
            cache: LruCache::new(LRU_CACHE_SIZE),
            semaphore: Arc::new(Semaphore::new(inflight_per_conn)),
        })
    }

    async fn prepare_statement(&mut self, query: Arc<str>) -> Result<Statement, DatabaseError> {
        let client: Arc<Client> = self.client.clone();

        self.cache
            .get_or_fetch(query, move |key: &Arc<str>| {
                let client: Arc<Client> = client.clone();
                let query: Arc<str> = key.clone();
                async move { client.prepare(&query).await.map_err(DatabaseError::Postgres) }
            })
            .await
    }

    pub async fn dispatch(&mut self) {
        while let Some(message) = self.receiver.recv().await {
            let Ok(permit) = self.semaphore.clone().acquire_owned().await else {
                break;
            };

            match message {
                ActorMessage::Execute { query, args, sender } => {
                    let statement: Statement = match self.prepare_statement(query.clone()).await {
                        Ok(statement) => statement,
                        Err(e) => {
                            sender.send(Err(e)).ok();
                            continue;
                        }
                    };

                    let client: Arc<Client> = self.client.clone();
                    tokio::spawn(async move {
                        let params: Vec<&(dyn ToSql + Sync)> = args.iter().map(|arg: &SqlArg| arg.as_sql()).collect();

                        let result: Result<RowSet, DatabaseError> = match client.query(&statement, &params).await {
                            Err(e) => Err(DatabaseError::Postgres(e)),
                            Ok(rows) => Ok(RowSet::from_pg_rows(rows)),
                        };

                        sender.send(result).ok();
                        drop(permit);
                    });
                }
            }
        }
    }
}
