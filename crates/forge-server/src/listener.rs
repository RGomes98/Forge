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
    pub threads: Option<usize>,
}

pub struct Listener<T> {
    state: Option<Arc<T>>,
    router: Arc<Router<T>>,
    options: ListenerOptions,
}

impl<T> Listener<T>
where
    T: Send + Sync + 'static,
{
    pub fn new(router: Router<T>, options: ListenerOptions) -> Self {
        Self {
            options,
            state: None,
            router: Arc::new(router),
        }
    }

    pub fn with_state(mut self, state: T) -> Self {
        self.state = Some(Arc::new(state));
        self
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

        let threads: usize = self.options.threads.filter(|&n: &usize| n >= 1).unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n: NonZero<usize>| n.get())
                .unwrap_or(1)
        });

        info!(threads, "Listener running on http://{address}");

        let handles: Vec<JoinHandle<()>> = (0..threads)
            .map(|i: usize| {
                let shared_router: Arc<Router<T>> = self.router.clone();
                let shared_state: Option<Arc<T>> = self.state.clone();

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
                                    let thread_router: Arc<Router<T>> = shared_router.clone();
                                    let thread_state: Option<Arc<T>> = shared_state.clone();

                                    if let Err(e) = stream.set_nodelay(true) {
                                        warn!("Failed to set 'TCP_NODELAY' on thread {i}: {e}");
                                    }

                                    monoio::spawn(async move {
                                        Self::handle_connection(stream, thread_router, thread_state).await;
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

    async fn handle_connection(stream: TcpStream, router: Arc<Router<T>>, state: Option<Arc<T>>) {
        let mut connection: Connection<T> = Connection { router, stream, state };
        let mut buffer: Vec<u8> = vec![0; BUFFER_SIZE];

        loop {
            match connection.process_request(buffer).await {
                Ok(connection_buffer) => buffer = connection_buffer,
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
