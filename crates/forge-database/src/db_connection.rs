use std::sync::Arc;

use super::RowSet;
use super::database::DbCommand;
use super::error::DatabaseError;
use super::sql_args::SqlArg;
use forge_utils::LruCache;
use tokio::sync::{Semaphore, mpsc::Receiver};
use tokio_postgres::tls::NoTlsStream;
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, Connection, Error, NoTls, Socket, Statement};

const LRU_CACHE_SIZE: usize = 256;

#[derive(Debug)]
pub struct DbConnection {
    client: Arc<Client>,
    semaphore: Arc<Semaphore>,
    receiver: Receiver<DbCommand>,
    cache: LruCache<Arc<str>, Statement>,
}

impl DbConnection {
    pub async fn new(
        database_url: String,
        inflight_per_conn: usize,
        receiver: Receiver<DbCommand>,
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
        let client: &Arc<Client> = &self.client;

        self.cache
            .get_or_fetch(query, move |key: &Arc<str>| {
                let client: Arc<Client> = client.clone();
                let query: Arc<str> = key.clone();
                async move { client.prepare(&query).await.map_err(DatabaseError::Postgres) }
            })
            .await
    }

    pub async fn process_queue(&mut self) {
        while let Some(cmd) = self.receiver.recv().await {
            let Ok(permit) = self.semaphore.clone().acquire_owned().await else {
                break;
            };

            match cmd {
                DbCommand::Execute { query, args, reply } => {
                    let statement: Statement = match self.prepare_statement(query.clone()).await {
                        Ok(statement) => statement,
                        Err(e) => {
                            reply.send(Err(e)).ok();
                            continue;
                        }
                    };

                    let client: Arc<Client> = self.client.clone();
                    tokio::spawn(async move {
                        let params: Vec<&(dyn ToSql + Sync)> = args.iter().map(|arg: &SqlArg| arg.as_sql()).collect();

                        let row_set: Result<RowSet, DatabaseError> = match client.query(&statement, &params).await {
                            Ok(rows) => Ok(RowSet::from_pg_rows(rows)),
                            Err(e) => Err(DatabaseError::Postgres(e)),
                        };

                        reply.send(row_set).ok();
                        drop(permit);
                    });
                }
            }
        }
    }
}
