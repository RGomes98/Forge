use std::{net::Ipv4Addr, sync::Arc};

use forge::prelude::*;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

struct State {
    db: Database,
    version: &'static str,
}

fn main() {
    let mut router: Router<State> = Router::new();

    let listener_options: ListenerOptions = ListenerOptions {
        threads: Config::from_env("THREADS").ok(),
        port: Config::from_env("PORT").unwrap_or(3000),
        host: Config::from_env("HOST").unwrap_or_else(|_| Ipv4Addr::new(127, 0, 0, 1)),
    };

    let database_options: DatabaseOptions = DatabaseOptions {
        url: Config::from_env("DB_URL").unwrap_or_default(),
        threads: Config::from_env("DB_THREADS").unwrap_or(8),
        inflight_per_conn: Config::from_env("DB_INFLIGHT_PER_CONN").unwrap_or(32),
    };

    let state: State = State {
        version: "Forge Example v0.1.0",
        db: Database::new(database_options).expect("failed to initialize database"),
    };

    router.register(ping);
    router.register(version);
    router.register(get_users);
    router.register(create_user);
    router.register(reset_database);
    router.register(populate_database);

    Listener::new(router, listener_options)
        .with_state(state)
        .run()
        .expect("failed to initialize server")
}

#[forge::get("/ping")]
async fn ping(_req: Request<'_>) -> Response<'static> {
    Response::new(HttpStatus::Ok).text("OK")
}

#[forge::get("/version")]
async fn version(_req: Request<'_>, state: Arc<State>) -> Response<'static> {
    Response::new(HttpStatus::Ok).text(state.version)
}

#[forge::get("/users")]
async fn get_users(_req: Request<'_>, state: Arc<State>) -> Response<'static> {
    match state.db.query("SELECT * FROM users", vec![]).await {
        Ok(users) => Response::new(HttpStatus::Ok).json(users.as_objects()),
        Err(e) => HttpError::new(HttpStatus::InternalServerError, e.to_string()).into(),
    }
}

#[forge::post("/user/:username")]
async fn create_user(req: Request<'_>, state: Arc<State>) -> Response<'static> {
    let Some(username) = req.params.get("username") else {
        return HttpError::new(HttpStatus::BadRequest, "missing parameter \"username\"").into();
    };

    let sql: &str = "INSERT INTO users (username) VALUES ($1) RETURNING *";
    let args: Vec<SqlArg> = vec![SqlArg::Text((*username).into())];

    match state.db.query(sql, args).await {
        Ok(user) => Response::new(HttpStatus::Created).json(user.as_objects()),
        Err(e) => HttpError::new(HttpStatus::InternalServerError, e.to_string()).into(),
    }
}

#[forge::post("/reset")]
async fn reset_database(_req: Request<'_>, state: Arc<State>) -> Response<'static> {
    if let Err(e) = state.db.query("DROP TABLE IF EXISTS users", vec![]).await {
        return HttpError::new(HttpStatus::InternalServerError, e.to_string()).into();
    }

    let sql: &str = r#"
    CREATE TABLE users (
        id BIGSERIAL PRIMARY KEY,
        username TEXT UNIQUE NOT NULL,
        active BOOLEAN DEFAULT true
    )
    "#;

    match state.db.query(sql, vec![]).await {
        Ok(..) => Response::new(HttpStatus::Ok).text("table \"users\" reseted successfully!"),
        Err(e) => HttpError::new(HttpStatus::InternalServerError, e.to_string()).into(),
    }
}

#[forge::post("/populate")]
async fn populate_database(_req: Request<'_>, state: Arc<State>) -> Response<'static> {
    let sql: &str = "INSERT INTO users (username, active) VALUES ($1, $2), ($3, $4)";

    let args: Vec<SqlArg> = vec![
        SqlArg::Text("john_doe".into()),
        SqlArg::Bool(false),
        SqlArg::Text("jane_doe".into()),
        SqlArg::Bool(false),
    ];

    match state.db.query(sql, args).await {
        Ok(..) => Response::new(HttpStatus::Created).text("database successfully seeded!"),
        Err(e) => HttpError::new(HttpStatus::InternalServerError, e.to_string()).into(),
    }
}
