use std::u64;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use uuid::Uuid;

async fn reset(State(state): State<QuoteBookState>) -> Response {
    info!("reset");
    sqlx::query!(r#"DELETE FROM quotes"#)
        .execute(&state.pool)
        .await
        .expect("unable to reset quotes");
    StatusCode::OK.into_response()
}

async fn cite(Path(id): Path<Uuid>, State(state): State<QuoteBookState>) -> Response {
    info!("cite - id={}", id);
    if let Ok(rec) = sqlx::query_as!(Quote, r#"SELECT * FROM quotes WHERE id = $1"#, id)
        .fetch_one(&state.pool)
        .await
    {
        Json(rec).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn remove(Path(id): Path<Uuid>, State(state): State<QuoteBookState>) -> Response {
    info!("remove - id={}", id);
    if let Ok(quote) = sqlx::query_as!(Quote, r#"DELETE FROM quotes WHERE id = $1 RETURNING *"#, id)
        .fetch_one(&state.pool)
        .await
    {
        (StatusCode::OK, Json(quote)).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn undo(
    Path(id): Path<Uuid>,
    State(state): State<QuoteBookState>,
    Json(quote): Json<Quote>,
) -> Response {
    info!("undo - id={}", id);
    if let Ok(quote) = sqlx::query_as!(
        Quote,
        r#"UPDATE quotes SET author = $1, quote = $2, version = version + 1 WHERE id = $3 RETURNING *"#,
        quote.author,
        quote.quote,
        id
    )
    .fetch_one(&state.pool)
    .await
    {
        Json(quote).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn draft(State(state): State<QuoteBookState>, Json(quote): Json<Quote>) -> Response {
    info!("draft - {:?}", quote);
    let rec: Quote = sqlx::query_as!(
        Quote,
        r#"
    INSERT INTO quotes (author, quote)
        VALUES ($1, $2)
        RETURNING *;
    "#,
        quote.author,
        quote.quote
    )
    .fetch_one(&state.pool)
    .await
    .expect("unable to insert quote");

    (StatusCode::CREATED, Json(rec)).into_response()
}

#[derive(Serialize)]
struct Page {
    quotes: Vec<Quote>,
    page: u64,
    next_token: Option<String>,
}

#[derive(Deserialize)]
struct PaginationId {
    token: Option<String>,
}

async fn list(State(state): State<QuoteBookState>, page: Query<PaginationId>) -> Response {
    let token = page.0.token;
    info!("list: {token:?}");
    let (page, quotes) = if let Some(token) = token {
        if !token.starts_with("0000") {
            return StatusCode::BAD_REQUEST.into_response();
        }

        let page = base62::decode(token).expect("unable to decode");
        let page = page as u64 ^ u64::MAX;
        info!(page);

        let quotes = sqlx::query_as!(
            Quote,
            "SELECT * FROM quotes ORDER BY created_at OFFSET $1 LIMIT 4",
            page as i64 * 3
        )
        .fetch_all(&state.pool)
        .await
        .expect("Unable to get data");
        (page + 1, quotes)
    } else {
        let quotes = sqlx::query_as!(Quote, "SELECT * FROM quotes ORDER BY created_at LIMIT 4")
            .fetch_all(&state.pool)
            .await
            .expect("Unable to get data");
        (1u64, quotes)
    };

    if quotes.len() < 4 {
        Json(Page {
            quotes: quotes,
            page,
            next_token: None,
        })
        .into_response()
    } else {
        Json(Page {
            quotes: quotes.into_iter().take(3).collect(),
            page,
            next_token: Some(format!("{:0>16}", base62::encode(page ^ u64::MAX))),
        })
        .into_response()
    }
}

#[derive(sqlx::FromRow, Deserialize, Serialize, PartialEq, Debug)]
struct Quote {
    id: Option<Uuid>,
    author: String,
    quote: String,
    created_at: Option<DateTime<Utc>>,
    version: Option<i32>,
}

#[derive(Clone)]
struct QuoteBookState {
    pool: sqlx::PgPool,
}

pub fn router(pool: sqlx::PgPool) -> Router {
    debug!("Loading routes");

    let state = QuoteBookState { pool };
    Router::new()
        .route("/reset", post(reset))
        .route("/cite/:id", get(cite))
        .route("/remove/:id", delete(remove))
        .route("/undo/:id", put(undo))
        .route("/draft", post(draft))
        .route("/list", get(list))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use serde_json::json;
    use sqlx::{query, query_as};

    use super::*;

    #[rstest::fixture]
    async fn pool() -> sqlx::PgPool {
        // TODO: figure out how to grab the port number automatically
        let pool =
            sqlx::PgPool::connect("postgres://postgres:postgres@localhost:24145/shuttlings-cch24")
                .await
                .unwrap();

        sqlx::migrate!()
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        pool
    }

    #[rstest::fixture]
    async fn server(#[future] pool: sqlx::PgPool) -> TestServer {
        let server = TestServer::new(router(pool.await)).unwrap();
        server
            .post("/reset")
            .json(&json!({"author":"Santa","quote":"Ho ho ho!"}))
            .await;
        server
    }
    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_reset(#[future] server: TestServer, #[future] pool: sqlx::PgPool) {
        let server = server.await;
        let pool = pool.await;
        server
            .post("/draft")
            .json(&json!({"author":"Santa","quote":"Ho ho ho!"}))
            .await;

        let res = server
            .post("/reset")
            .json(&json!({"author":"Santa","quote":"Ho ho ho!"}))
            .await;

        res.assert_status_success();

        let db_quote = query_as!(Quote, r#"SELECT * FROM quotes"#,)
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(0, db_quote.len())
    }
    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_draft(#[future] server: TestServer, #[future] pool: sqlx::PgPool) {
        let server = server.await;
        let pool = pool.await;
        let res = server
            .post("/draft")
            .json(&json!({"author":"Santa","quote":"Ho ho ho!"}))
            .await;

        debug!(?res);
        res.assert_status(StatusCode::CREATED);
        res.assert_json_contains(&json!({"author":"Santa","quote":"Ho ho ho!"}));
        let quote: Quote = res.json();
        assert!(quote.id.is_some());
        assert!(quote.created_at.is_some());
        assert!(quote.version.is_some());

        let db_quote = query_as!(
            Quote,
            r#"SELECT * FROM quotes WHERE id = $1"#,
            quote.id.unwrap()
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(quote, db_quote);
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_cite(#[future] server: TestServer) {
        let server = server.await;
        let res = server
            .post("/draft")
            .json(&json!({"author":"Santa","quote":"Ho ho ho!"}))
            .await;

        debug!(?res);
        res.assert_status(StatusCode::CREATED);
        let expected_quote: Quote = res.json();

        let res = server
            .get(&format!("/cite/{}", expected_quote.id.unwrap()))
            .await;
        debug!(?res);

        res.assert_status_success();
        assert_eq!(expected_quote, res.json());
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_cite_not_found(#[future] server: TestServer) {
        let server = server.await;

        let res = server.get(&format!("/cite/{}", Uuid::default())).await;
        debug!(?res);

        res.assert_status_not_found();
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_remove(#[future] server: TestServer, #[future] pool: sqlx::PgPool) {
        let server = server.await;
        let pool = pool.await;
        let res = server
            .post("/draft")
            .json(&json!({"author":"Santa","quote":"Ho ho ho!"}))
            .await;

        debug!(?res);
        res.assert_status(StatusCode::CREATED);
        let expected_quote: Quote = res.json();

        let res = server
            .delete(&format!("/remove/{}", expected_quote.id.unwrap()))
            .await;
        debug!(?res);

        res.assert_status_success();
        res.assert_text_contains(format!("{}", expected_quote.id.unwrap()));

        let query = query!(
            "SELECT * FROM quotes WHERE id = $1",
            expected_quote.id.unwrap()
        )
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(query.is_none());
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_remove_not_found(#[future] server: TestServer) {
        let server = server.await;

        let res = server.delete(&format!("/remove/{}", Uuid::default())).await;
        debug!(?res);

        res.assert_status_not_found();
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_undo(#[future] server: TestServer, #[future] pool: sqlx::PgPool) {
        let server = server.await;
        let pool = pool.await;
        let res = server
            .post("/draft")
            .json(&json!({"author":"Santa","quote":"Ho ho ho!"}))
            .await;

        debug!(?res);
        res.assert_status(StatusCode::CREATED);
        let expected_quote: Quote = res.json();

        let res = server
            .put(&format!("/undo/{}", expected_quote.id.unwrap()))
            .json(&json!({"author":"Paul","quote":"Oh!"}))
            .await;
        debug!(?res);

        res.assert_status_success();
        res.assert_json_contains(&json!({"author":"Paul","quote":"Oh!"}));

        let result: Quote = res.json();

        let db_quote = query_as!(
            Quote,
            r#"SELECT * FROM quotes WHERE id = $1"#,
            expected_quote.id.unwrap()
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(result, db_quote);
        assert_eq!(2, db_quote.version.unwrap())
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_undo_not_found(#[future] server: TestServer) {
        let server = server.await;

        let res = server
            .put(&format!("/undo/{}", Uuid::default()))
            .json(&json!({"author":"Paul","quote":"Oh!"}))
            .await;
        debug!(?res);

        res.assert_status_not_found();
    }
}
