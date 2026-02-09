use forge::prelude::*;
use std::sync::Arc;

pub const VERSION: &str = "Forge Example v0.1.0";

#[forge::get("/version")]
#[allow(clippy::redundant_allocation)]
pub async fn version_handler(_req: Request<'_>, state: Arc<&'static str>) -> Response<'static> {
    Response::new(HttpStatus::Ok).text(*state)
}
