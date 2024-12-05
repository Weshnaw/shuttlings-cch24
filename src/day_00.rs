use axum::{http::{header, HeaderName, StatusCode}, routing::get, Router};
use tracing::{debug, instrument};

#[instrument]
pub async fn hello_world() -> &'static str {
    debug!("Calling hello_world");
    "Hello, bird!"
}

#[instrument]
async fn seek() -> (StatusCode, [(HeaderName, &'static str); 1]) {
    debug!("Calling seek");
    (StatusCode::FOUND, [(header::LOCATION, "https://www.youtube.com/watch?v=9Gc4QTqslN4")])
}


#[instrument]
pub fn router() -> Router {
    debug!("Loading negone routes");
    Router::new()
        .route("/seek", get(seek))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hello() {
        assert_eq!("Hello, bird!", hello_world().await);
    }
    
    #[tokio::test]
    async fn test_seek() {
        let (status, headers) = seek().await;

        assert_eq!(StatusCode::from_u16(302).unwrap(), status);
        assert_eq!("https://www.youtube.com/watch?v=9Gc4QTqslN4", headers[0].1);
        assert_eq!("location", headers[0].0);
    }
}
