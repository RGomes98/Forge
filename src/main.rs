use std::{net::Ipv4Addr, time::Duration};

use forge::prelude::*;
use monoio::time;

mod poc;
use poc::{VERSION, version_handler};

#[monoio::main]
async fn main() {
    let mut router: Router<&'static str> = Router::new();

    let options: ListenerOptions = ListenerOptions {
        threads: Config::from_env("THREADS").ok(),
        port: Config::from_env("PORT").unwrap_or(3000),
        host: Config::from_env("HOST").unwrap_or_else(|_| Ipv4Addr::new(127, 0, 0, 1)),
    };

    router.register(user_handler());
    router.register(ping_handler());
    router.register(store_handler());
    router.register(health_handler());
    router.register(version_handler());

    Listener::new(router, options)
        .with_default_logger()
        .with_state(VERSION)
        .run();
}

#[forge::get("/ping")]
async fn ping_handler(req: Request<'_>) -> Response<'_> {
    let headers: Headers = req.headers;
    println!("Headers: {headers:#?}");
    Response::new(HttpStatus::Ok).text("pong!")
}

#[forge::get("/health")]
async fn health_handler(_req: Request<'_>) -> Response<'_> {
    Response::new(HttpStatus::Ok).text("OK")
}

#[forge::get("/user")]
async fn user_handler(_req: Request<'_>) -> Response<'_> {
    time::sleep(Duration::from_secs(5)).await;
    let user: serde_json::Value = serde_json::json!({ "name": "john doe", "age": 18 });
    Response::new(HttpStatus::Ok).json(user)
}

#[forge::get("/store/:store_id/customer/:customer_id")]
async fn store_handler(req: Request<'_>) -> Response<'_> {
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
