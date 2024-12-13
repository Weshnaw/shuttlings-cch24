use axum::{routing::get, Router};
use shuttlings_cch24::{day_00, day_02, day_05, day_09, day_12};

#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let router = Router::new()
        .route("/", get(day_00::hello_world))
        .nest("/-1", day_00::router())
        .nest("/2", day_02::router())
        .nest("/5", day_05::router())
        .nest_service("/9", day_09::router())
        .nest_service("/12", day_12::router());

    Ok(router.into())
}

