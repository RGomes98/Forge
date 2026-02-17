use std::io::Error;
use std::net::{Ipv4Addr, SocketAddr};
use std::num::NonZero;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use super::{Connection, ListenerError};
use forge_http::Response;
use forge_router::Router;
use monoio::net::{TcpListener, TcpStream};
use monoio::time::TimeDriver;
use monoio::{FusionDriver, FusionRuntime, IoUringDriver, LegacyDriver, RuntimeBuilder};

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

    pub fn run(self) -> Result<(), ListenerError> {
        let addr: SocketAddr = SocketAddr::from((self.options.host, self.options.port));

        let threads: usize = self.options.threads.filter(|&n: &usize| n >= 1).unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n: NonZero<usize>| n.get())
                .unwrap_or(1)
        });

        println!("Listener running on http://{addr}");
        let handles: Vec<JoinHandle<Result<(), ListenerError>>> = (0..threads)
            .map(|idx: usize| {
                let shared_router: Arc<Router<T>> = self.router.clone();
                let shared_state: Option<Arc<T>> = self.state.clone();

                thread::spawn(move || -> Result<(), ListenerError> {
                    let mut runtime: FusionRuntime<TimeDriver<IoUringDriver>, TimeDriver<LegacyDriver>> =
                        RuntimeBuilder::<FusionDriver>::new()
                            .enable_all()
                            .with_entries(DEFAULT_RING_ENTRIES)
                            .build()
                            .map_err(|e: Error| ListenerError::Runtime(idx, e))?;

                    runtime.block_on(async {
                        let listener: TcpListener =
                            TcpListener::bind(addr).map_err(|e: Error| ListenerError::Bind(addr, idx, e))?;

                        loop {
                            match listener.accept().await {
                                Ok((stream, _)) => {
                                    let thread_router: Arc<Router<T>> = shared_router.clone();
                                    let thread_state: Option<Arc<T>> = shared_state.clone();

                                    if let Err(e) = stream.set_nodelay(true) {
                                        eprintln!("Failed to set 'TCP_NODELAY' on worker #{idx}: {e:?}");
                                    }

                                    monoio::spawn(async move {
                                        Self::handle_connection(stream, thread_router, thread_state).await;
                                    });
                                }
                                Err(e) => {
                                    eprintln!("Failed to accept connection on worker #{idx}: {e:?}");
                                }
                            }
                        }

                        #[allow(unreachable_code)]
                        Ok(())
                    })
                })
            })
            .collect();

        for (idx, handler) in handles.into_iter().enumerate() {
            match handler.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e),
                Err(e) => {
                    let msg: &str = e.downcast_ref::<&'static str>().copied().unwrap_or("unknown cause");
                    return Err(ListenerError::ThreadPanic(idx, msg.into()));
                }
            }
        }

        Ok(())
    }

    async fn handle_connection(stream: TcpStream, router: Arc<Router<T>>, state: Option<Arc<T>>) {
        let mut connection: Connection<T> = Connection { router, stream, state };
        let mut buffer: Vec<u8> = vec![0; BUFFER_SIZE];

        loop {
            match connection.process_request(buffer).await {
                Ok(connection_buffer) => buffer = connection_buffer,
                Err(ListenerError::ConnectionClosed) => break,
                Err(ListenerError::Http(e)) => {
                    Response::new(e.status).send(&mut connection.stream).await.ok();
                    break;
                }
                Err(_) => unreachable!(),
            }
        }
    }
}
