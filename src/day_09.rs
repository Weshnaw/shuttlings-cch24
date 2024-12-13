#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use leaky_bucket::RateLimiter;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

#[derive(Clone, Debug)]
struct MilkState {
    limiter: Arc<RwLock<RateLimiter>>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
enum MilkUnit {
    Liters(f32),
    Gallons(f32),
    Litres(f32),
    Pints(f32),
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct MilkConversion {
    #[serde(flatten)]
    unit: MilkUnit,
}

#[instrument]
async fn milk(State(state): State<MilkState>, headers: HeaderMap, body: String) -> Response {
    debug!("Calling milk");
    let lock = state.limiter.read().await;
    let limted = lock.try_acquire(1);
    drop(lock);
    if limted {
        match headers.get(CONTENT_TYPE).map(|x| x.as_bytes()) {
            Some(b"application/json") => {
                debug!(?body);
                if let Ok(conversion) = serde_json::from_str::<MilkConversion>(&body) {
                    let unit = match conversion.unit {
                        MilkUnit::Liters(liters) => MilkUnit::Gallons(liters * 0.264_172_05),
                        MilkUnit::Gallons(gallons) => MilkUnit::Liters(gallons * 3.785_411_8),
                        MilkUnit::Litres(litres) => MilkUnit::Pints(litres * 1.759_754),
                        MilkUnit::Pints(pints) => MilkUnit::Litres(pints * 0.568_261_25),
                    };
                    info!(?unit);
                    let converted = MilkConversion { unit };

                    (StatusCode::OK, Json(converted)).into_response()
                } else {
                    (StatusCode::BAD_REQUEST).into_response()
                }
            }
            _ => (StatusCode::OK, "Milk withdrawn\n").into_response(),
        }
    } else {
        (StatusCode::TOO_MANY_REQUESTS, "No milk available\n").into_response()
    }
}

fn default_limiter() -> RateLimiter {
    RateLimiter::builder()
        .max(5)
        .initial(5)
        .interval(Duration::from_secs(1))
        .build()
}

async fn refill(State(state): State<MilkState>) -> Response {
    debug!("Calling refill");
    let mut lock = state.limiter.write().await;
    *lock = default_limiter();
    drop(lock);
    StatusCode::OK.into_response()
}

#[instrument]
pub fn router() -> Router {
    debug!("Loading routes");

    let limiter = Arc::new(RwLock::new(default_limiter()));

    let state = MilkState { limiter };

    Router::new()
        .route("/milk", post(milk))
        .route("/refill", post(refill))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use serde_json::Value;
    use tokio::time::sleep;

    use super::*;

    async fn check_milk_request(server: &TestServer, expected_status: StatusCode) {
        let result = server.post(&"/milk").await;

        assert_eq!(expected_status, result.status_code());
        match expected_status {
            StatusCode::OK => assert_eq!("Milk withdrawn\n", result.text()),
            StatusCode::TOO_MANY_REQUESTS => assert_eq!("No milk available\n", result.text()),
            _ => panic!("Not valid status"),
        };
    }

    #[test_log::test(tokio::test)]
    async fn test_milk_single() {
        let server = TestServer::new(router()).unwrap();
        check_milk_request(&server, StatusCode::OK).await;
    }

    #[test_log::test(tokio::test)]
    async fn test_milk_to_many_requests() {
        let server = TestServer::new(router()).unwrap();
        for _ in 0..5 {
            check_milk_request(&server, StatusCode::OK).await;
        }
        check_milk_request(&server, StatusCode::TOO_MANY_REQUESTS).await;
        sleep(Duration::from_secs(1)).await;
        check_milk_request(&server, StatusCode::OK).await;
        check_milk_request(&server, StatusCode::TOO_MANY_REQUESTS).await;
    }

    #[test_log::test(tokio::test)]
    async fn test_milk_invalid_json() {
        let server = TestServer::new(router()).unwrap();
        let result = server
            .post(&"/milk")
            .json(&serde_json::from_str::<Value>(r#"{"liters":1,"gallons":5}"#).unwrap())
            .await;

        debug!(?result);
        assert_eq!(StatusCode::BAD_REQUEST, result.status_code());
    }

    async fn milk_conversion_request(
        server: &TestServer,
        unit: MilkUnit,
        expected_status: StatusCode,
        expected_unit: MilkUnit,
    ) {
        let result = server.post(&"/milk").json(&MilkConversion { unit }).await;

        debug!(?result);
        assert_eq!(expected_status, result.status_code());
        match expected_status {
            StatusCode::OK => assert_eq!(
                MilkConversion {
                    unit: expected_unit,
                },
                result.json::<MilkConversion>()
            ),
            StatusCode::TOO_MANY_REQUESTS => assert_eq!("No milk available\n", result.text()),
            StatusCode::BAD_REQUEST => assert_eq!("", result.text()),
            _ => panic!("Not valid status"),
        };
    }

    #[test_log::test(tokio::test)]
    async fn test_milk_convert_liters() {
        let server = TestServer::new(router()).unwrap();

        milk_conversion_request(
            &server,
            MilkUnit::Liters(5.0),
            StatusCode::OK,
            MilkUnit::Gallons(1.3208603),
        )
        .await;
    }

    #[test_log::test(tokio::test)]
    async fn test_milk_convert_gallons() {
        let server = TestServer::new(router()).unwrap();

        milk_conversion_request(
            &server,
            MilkUnit::Gallons(5.0),
            StatusCode::OK,
            MilkUnit::Liters(18.92706),
        )
        .await;
    }

    #[test_log::test(tokio::test)]
    async fn test_milk_convert_rate_limited() {
        let server = TestServer::new(router()).unwrap();

        for _ in 0..5 {
            milk_conversion_request(
                &server,
                MilkUnit::Gallons(5.0),
                StatusCode::OK,
                MilkUnit::Liters(18.92706),
            )
            .await;
        }
        milk_conversion_request(
            &server,
            MilkUnit::Gallons(5.0),
            StatusCode::TOO_MANY_REQUESTS,
            MilkUnit::Liters(18.92706),
        )
        .await;
    }

    #[test_log::test(tokio::test)]
    async fn test_milk_refill() {
        let server = TestServer::new(router()).unwrap();
        for _ in 0..5 {
            check_milk_request(&server, StatusCode::OK).await;
        }
        check_milk_request(&server, StatusCode::TOO_MANY_REQUESTS).await;
        let result = server.post(&"/refill").await;
        assert_eq!(StatusCode::OK, result.status_code());
        for _ in 0..5 {
            check_milk_request(&server, StatusCode::OK).await;
        }
        check_milk_request(&server, StatusCode::TOO_MANY_REQUESTS).await;
    }
}
