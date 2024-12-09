#![allow(dead_code)]

use axum::{routing::get, Router};
use tracing::{debug, instrument};

#[instrument]
async fn hello() -> &'static str {
    debug!("Calling hello_world");
    "Hello, bird!"
}

#[instrument]
pub fn router() -> Router {
    debug!("Loading routes");
    Router::new().route("/hello", get(hello))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_() {}
}
