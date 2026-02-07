use std::io::Error;
use std::net::{Ipv4Addr, SocketAddr};
use std::num::NonZero;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use super::{Connection, ListenerError};
use forge_http::Response;
use forge_logging::init_logger;
use forge_router::Router;
use monoio::net::{TcpListener, TcpStream};
use monoio::time::TimeDriver;
use monoio::{FusionDriver, FusionRuntime, IoUringDriver, LegacyDriver, RuntimeBuilder};
use tracing::{error, info, warn};

const DEFAULT_RING_ENTRIES: u32 = 4096;
const BUFFER_SIZE: usize = 4096;

pub struct ListenerOptions {
    pub port: u16,
    pub host: Ipv4Addr,
    pub threads: usize,
}

pub struct Listener {
    router: Arc<Router>,
    options: ListenerOptions,
}

impl Listener {
    pub fn new(router: Router, options: ListenerOptions) -> Self {
        Self {
            options,
            router: Arc::new(router),
        }
    }

    pub fn with_default_logger(self) -> Self {
        match init_logger() {
            Ok(_) => info!("Default logger initialized successfully"),
            Err(_) => warn!("Logger already initialized, using existing global subscriber"),
        };

        self
    }

    pub fn run(self) {
        let address: SocketAddr = SocketAddr::from((self.options.host, self.options.port));
        let shared_router: Arc<Router> = self.router;

        info!("Listener running on http://{address}");

        let threads: usize = if self.options.threads == 0 {
            thread::available_parallelism()
                .map(|n: NonZero<usize>| n.get())
                .unwrap_or_else(|e: Error| {
                    warn!("Failed to determine available parallelism: {e}. Defaulting to 1 thread.");
                    1
                })
        } else {
            self.options.threads
        };

        let handles: Vec<JoinHandle<()>> = (0..threads)
            .map(|i: usize| {
                let router_ref: Arc<Router> = shared_router.clone();

                thread::spawn(move || {
                    let mut runtime: FusionRuntime<TimeDriver<IoUringDriver>, TimeDriver<LegacyDriver>> =
                        match RuntimeBuilder::<FusionDriver>::new()
                            .enable_all()
                            .with_entries(DEFAULT_RING_ENTRIES)
                            .build()
                        {
                            Ok(runtime) => runtime,
                            Err(e) => {
                                error!("Failed to build Monoio runtime for thread {i}: {e}");
                                return;
                            }
                        };

                    runtime.block_on(async {
                        let listener: TcpListener = match TcpListener::bind(address) {
                            Ok(listener) => listener,
                            Err(e) => {
                                error!("Bind error on thread {i}: {e}");
                                return;
                            }
                        };

                        loop {
                            match listener.accept().await {
                                Ok((stream, _address)) => {
                                    let thread_router: Arc<Router> = router_ref.clone();

                                    if let Err(e) = stream.set_nodelay(true) {
                                        warn!("Failed to set 'TCP_NODELAY' on thread {i}: {e}");
                                    }

                                    monoio::spawn(async move {
                                        Self::handle_connection(stream, thread_router).await;
                                    });
                                }
                                Err(e) => {
                                    error!("Failed to accept connection on thread {i}: {e}");
                                }
                            }
                        }
                    });
                })
            })
            .collect();

        for (i, handler) in handles.into_iter().enumerate() {
            if let Err(e) = handler.join() {
                error!("Thread {i} failed to join: {e:?}");
            }
        }
    }

    async fn handle_connection(stream: TcpStream, router: Arc<Router>) {
        let mut connection: Connection = Connection { router, stream };
        let mut buffer: Vec<u8> = vec![0; BUFFER_SIZE];

        loop {
            match connection.process_request(buffer).await {
                Ok(connection_buffer) => {
                    buffer = connection_buffer;
                }
                Err(e) => match e {
                    ListenerError::ConnectionClosed => break,
                    ListenerError::Http(e) => {
                        Response::new(e.status).send(&mut connection.stream).await.ok();
                        break;
                    }
                },
            }
        }
    }
}
