use std::{net::Ipv4Addr, sync::Arc, time::Duration};

use forge::prelude::*;
use monoio::time;

pub struct State {
    pub db: PgActor,
    pub version: &'static str,
}

#[monoio::main]
async fn main() {
    let mut router: Router<State> = Router::new();

    let listener_options: ListenerOptions = ListenerOptions {
        threads: Config::from_env("THREADS").ok(),
        port: Config::from_env("PORT").unwrap_or(3000),
        host: Config::from_env("HOST").unwrap_or_else(|_| Ipv4Addr::new(127, 0, 0, 1)),
    };

    let database_options: PgOptions = PgOptions {
        pool_size: Config::from_env("DB_POOL_SIZE").unwrap_or(8),
        database_url: Config::from_env("DB_URL").unwrap_or_default(),
        inflight_per_conn: Config::from_env("DB_INFLIGHT_PER_CONN").unwrap_or(32),
    };

    let state: State = State {
        version: "Forge Example v0.1.0",
        db: PgActor::new(database_options).expect("failed to initialize database"),
    };

    router.register(api_version_handler);
    router.register(db_version_handler);
    router.register(ping_handler);
    router.register(health_handler);
    router.register(user_handler);
    router.register(store_handler);

    if let Err(e) = Listener::new(router, listener_options).with_state(state).run() {
        eprintln!("failed to initialize server {e}")
    }
}

#[forge::get("/api/version")]
pub async fn api_version_handler(_req: Request<'_>, state: Arc<State>) -> Response<'static> {
    Response::new(HttpStatus::Ok).text(state.version)
}

#[forge::get("/db/version")]
pub async fn db_version_handler(_req: Request<'_>, state: Arc<State>) -> Response<'static> {
    match state.db.query("SELECT version()", vec![]).await {
        Ok(version) => Response::new(HttpStatus::Ok).json(version.as_objects()),
        Err(e) => HttpError::new(HttpStatus::InternalServerError, e.to_string()).into(),
    }
}

#[forge::get("/ping")]
async fn ping_handler(req: Request<'_>) -> Response<'static> {
    let headers: Headers = req.headers;
    println!("Headers: {headers:#?}");
    Response::new(HttpStatus::Ok).text("pong!")
}

#[forge::get("/health")]
async fn health_handler(_req: Request<'_>) -> Response<'static> {
    Response::new(HttpStatus::Ok).text("OK")
}

#[forge::get("/user")]
async fn user_handler(_req: Request<'_>) -> Response<'static> {
    time::sleep(Duration::from_secs(5)).await;
    Response::new(HttpStatus::Ok).body(r#"{"name":"John Doe","age":18}"#)
}

#[forge::get("/store/:store_id/customer/:customer_id")]
async fn store_handler(req: Request<'_>) -> Response<'static> {
    let Some(id_str) = req.params.get("store_id") else {
        return HttpError::new(HttpStatus::BadRequest, "missing parameter \"store_id\"").into();
    };

    let Ok(store_id) = id_str.parse::<i32>() else {
        return HttpError::new(HttpStatus::BadRequest, "parameter \"store_id\" must be a valid integer").into();
    };

    if store_id < 1 {
        return HttpError::new(HttpStatus::BadRequest, "parameter \"store_id\" must be greater than 0").into();
    }

    Response::new(HttpStatus::Ok).json(req.params)
}
