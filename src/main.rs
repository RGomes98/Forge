use std::{net::Ipv4Addr, time::Duration};

use forge::prelude::*;
use serde_json::json;
use tokio::time::sleep;

#[monoio::main]
async fn main() {
    let mut router: Router = Router::new();

    let options: ListenerOptions = ListenerOptions {
        host: Config::from_env("HOST").unwrap_or_else(|_| Ipv4Addr::new(127, 0, 0, 1)),
        threads: Config::from_env("THREADS").unwrap_or(0),
        port: Config::from_env("PORT").unwrap_or(3000),
    };

    routes!(router, {
        get "/user" => user_handler,
        get "/ping" => ping_handler,
        get "/health" => health_handler,
    });

    get!(router, "/store/:store_id/customer/:customer_id", store_handler);

    Listener::new(router, options).with_default_logger().run();
}

async fn user_handler(_: Request<'_>) -> Response<'_> {
    sleep(Duration::from_secs(5)).await;
    let user: serde_json::Value = json!({ "name": "john doe", "age": 18 });
    Response::new(HttpStatus::Ok).json(user)
}

fn ping_handler(req: Request) -> Response {
    let headers: Headers = req.headers;
    println!("Headers: {headers:#?}");
    Response::new(HttpStatus::Ok).text("pong!")
}

fn health_handler(_: Request) -> Response {
    Response::new(HttpStatus::Ok).text("OK")
}

fn store_handler(req: Request) -> Response {
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
