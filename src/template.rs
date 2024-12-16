#![allow(dead_code)]

use axum::{
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tracing::debug;

async fn hello() -> Response {
    debug!("Calling hello");
    "Hello, bird!".into_response()
}

pub fn router() -> Router {
    debug!("Loading routes");
    Router::new()
        .route("/hello", get(hello))
}

#[cfg(test)]
mod tests {
    use axum_test::TestServer;

    use super::*;

    #[rstest::fixture]
    fn server() -> TestServer {
        TestServer::new(router()).unwrap()
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_board(_server: TestServer) {
    }
}

