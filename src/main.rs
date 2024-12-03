use axum::{http::{header, HeaderName, StatusCode}, routing::get, Router};

async fn hello_world() -> &'static str {
    "Hello, bird!"
}

async fn seek() -> (StatusCode, [(HeaderName, &'static str); 1]) {
    (StatusCode::FOUND, [(header::LOCATION, "https://www.youtube.com/watch?v=9Gc4QTqslN4")])
}

#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let router = Router::new()
        .route("/", get(hello_world))
        .route("/-1/seek", get(seek));

    Ok(router.into())
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
