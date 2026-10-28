//! Reference server for the benchmark runner (`bench/runner`): the same routes as
//! `forge-server`, written with axum on a multi-threaded tokio runtime.
//!
//! Configuration comes from `BENCH_PORT` (default 18081) and `BENCH_THREADS` (default: all cores).

use std::env;
use std::net::Ipv4Addr;
use std::thread;

use axum::extract::Path;
use axum::routing::get;
use axum::serve::ListenerExt;
use axum::{Json, Router};
use mimalloc::MiMalloc;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::runtime::Builder;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Serialize)]
struct Message {
    message: &'static str,
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let port: u16 = env_or("BENCH_PORT", 18081);
    let threads: usize = env_or("BENCH_THREADS", thread::available_parallelism().map(|n| n.get()).unwrap_or(1));

    let app: Router = Router::new()
        .route("/ping", get(|| async { "OK" }))
        .route(
            "/json",
            get(|| async {
                Json(Message {
                    message: "Hello, World!",
                })
            }),
        )
        .route("/users/{id}", get(|Path(id): Path<String>| async move { format!("user {id}") }));

    Builder::new_multi_thread()
        .worker_threads(threads)
        .enable_all()
        .build()
        .expect("failed to build the tokio runtime")
        .block_on(async {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
                .await
                .expect("failed to bind")
                // Same as Forge, which enables TCP_NODELAY on every accepted connection.
                .tap_io(|tcp| {
                    let _ = tcp.set_nodelay(true);
                });

            axum::serve(listener, app).await.expect("server error");
        });
}
