use axum::{routing::get, Router};
use shuttlings_cch24::{day_00, day_02, day_05, day_09, day_12, day_16, day_19, day_23};
use tower_http::services::ServeDir;

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: sqlx::PgPool
) -> shuttle_axum::ShuttleAxum {

    sqlx::migrate!().run(&pool).await.expect("Failed to run migrations");

    let router = Router::new()
        .route("/", get(day_00::hello_world))
        .nest_service("/assets", ServeDir::new("assets"))
        .nest("/-1", day_00::router())
        .nest("/2", day_02::router())
        .nest("/5", day_05::router())
        .nest_service("/9", day_09::router())
        .nest_service("/12", day_12::router())
        .nest_service("/16", day_16::router())
        .nest_service("/19", day_19::router(pool.clone()))
        .nest_service("/23", day_23::router());

    Ok(router.into())
}

