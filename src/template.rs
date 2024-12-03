use axum::Router;
use tracing::{debug, instrument};


#[instrument]
pub fn router() -> Router {
    debug!("Loading routes");
    Router::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_() {
    }
}
