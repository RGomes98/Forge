//! Minimal Forge server used by the benchmark runner (`bench/runner`).
//!
//! It has no database and no state, so the numbers measure the HTTP stack only.
//! Configuration comes from `BENCH_PORT` (default 18080) and `BENCH_THREADS` (default: all cores).

use std::net::Ipv4Addr;

use forge::prelude::*;
use mimalloc::MiMalloc;
use serde::Serialize;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Serialize)]
struct Message {
    message: &'static str,
}

fn main() {
    let mut router: Router<()> = Router::new();

    let options: ListenerOptions = ListenerOptions {
        port: Config::from_env("BENCH_PORT").unwrap_or(18080),
        host: Ipv4Addr::LOCALHOST,
        threads: Config::from_env("BENCH_THREADS").ok(),
    };

    router.register(ping);
    router.register(json);
    router.register(user);

    Listener::new(router, options)
        .run()
        .expect("failed to run the benchmark server")
}

#[forge::get("/ping")]
async fn ping() -> Response<'static> {
    Response::new(HttpStatus::Ok).text("OK")
}

#[forge::get("/json")]
async fn json() -> Response<'static> {
    Response::new(HttpStatus::Ok).json(Message {
        message: "Hello, World!",
    })
}

#[forge::get("/users/:id")]
async fn user(request: Request<'_>) -> Response<'static> {
    match request.params.get("id") {
        Some(id) => Response::new(HttpStatus::Ok).text(format!("user {id}")),
        None => Response::new(HttpStatus::BadRequest),
    }
}
