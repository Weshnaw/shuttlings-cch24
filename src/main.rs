use axum::{routing::get, Router};
use shuttlings_cch24::negone;

#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let router = Router::new()
        .route("/", get(negone::hello_world))
        .nest("/-1", negone::router());

    Ok(router.into())
}

