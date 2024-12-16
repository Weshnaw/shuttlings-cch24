#![allow(dead_code)]

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::CookieJar;
use jsonwebtoken::{encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde_json::Value;
use tracing::debug;

const SECRET: &str = "SECRET";

async fn wrap(Json(payload): Json<Value>) -> Response {
    debug!("Calling wrap");

    let value = encode(
        &Header::default(),
        &payload,
        &EncodingKey::from_secret(SECRET.as_ref()),
    )
    .expect("Failed to encode jwt");

    [(header::SET_COOKIE, format!("gift={value}"))].into_response()
}

async fn unwrap(jar: CookieJar) -> Response {
    debug!("Calling unwrap");
    let Some(jwt) = jar.get("gift") else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let mut validation = Validation::default();
    validation.set_required_spec_claims::<&str>(&[]);

    let value = jsonwebtoken::decode::<Value>(
        jwt.value(),
        &DecodingKey::from_secret(SECRET.as_ref()),
        &validation,
    )
    .expect("Failed to decode token");

    Json(value.claims).into_response()
}

async fn decode(data: String) -> Response {
    let header = jsonwebtoken::decode_header(&data);
    debug!(?header);

    let pem = include_bytes!("day16_santa_public_key.pem");

    let key = DecodingKey::from_rsa_pem(pem).expect("Unable to create decoding key");

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_required_spec_claims::<&str>(&[]);
    validation.algorithms = vec![Algorithm::RS256, Algorithm::RS384, Algorithm::RS512];

    let value = jsonwebtoken::decode::<Value>(&data, &key, &validation);

    debug!(?value);
    match value {
        Ok(val) => Json(val.claims).into_response(),
        Err(err) => match err.kind() {
            jsonwebtoken::errors::ErrorKind::InvalidSignature => StatusCode::UNAUTHORIZED.into_response(),
            _ => StatusCode::BAD_REQUEST.into_response() 
        },
    }
}

pub fn router() -> Router {
    debug!("Loading routes");
    Router::new()
        .route("/wrap", post(wrap))
        .route("/unwrap", get(unwrap))
        .route("/decode", post(decode))
}

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use serde_json::Value;

    use super::*;

    #[rstest::fixture]
    fn server() -> TestServer {
        TestServer::new(router()).unwrap()
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_wrap(server: TestServer) {
        let response = server
            .post("/wrap")
            .json(&serde_json::from_str::<Value>(r#"{"cookie is delicious?": true}"#).unwrap())
            .await;

        debug!(?response);

        response.assert_status_success();
        let gift = response.cookie("gift");

        let response = server.get("/unwrap").add_cookie(gift).await;

        response.assert_status_success();
        response.assert_json(
            &serde_json::from_str::<Value>(r#"{"cookie is delicious?": true}"#).unwrap(),
        );
    }

    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn test_missing_cookie(server: TestServer) {
        let response = server.get("/unwrap").await;

        response.assert_status_bad_request();
    }

    #[rstest::rstest]
    #[case(
        "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.eyJyZWluZGVlclNuYWNrIjoiY2Fycm90cyIsInNhbnRhSGF0Q29sb3IiOiJyZWQiLCJzbm93R2xvYmVDb2xsZWN0aW9uIjo1LCJzdG9ja2luZ1N0dWZmZXJzIjpbInlvLXlvIiwiY2FuZHkiLCJrZXljaGFpbiJdLCJ0cmVlSGVpZ2h0Ijo3fQ.EoWSlwZIMHdtd96U_FkfQ9SkbzskSvgEaRpsUeZQFJixDW57vZud_k-MK1R1LEGoJRPGttJvG_5ewdK9O46OuaGW4DHIOWIFLxSYFTJBdFMVmAWC6snqartAFr2U-LWxTwJ09WNpPBcL67YCx4HQsoGZ2mxRVNIKxR7IEfkZDhmpDkiAUbtKyn0H1EVERP1gdbzHUGpLd7wiuzkJnjenBgLPifUevxGPgj535cp8I6EeE4gLdMEm3lbUW4wX_GG5t6_fDAF4URfiAOkSbiIW6lKcSGD9MBVEGps88lA2REBEjT4c7XHw4Tbxci2-knuJm90zIA9KX92t96tF3VFKEA", 
        StatusCode::OK, Some(r#"{"reindeerSnack":"carrots","santaHatColor":"red","snowGlobeCollection":5,"stockingStuffers":["yo-yo","candy","keychain"],"treeHeight":7}"#))]
    #[case(
        "eyJ0eXAiOiJKV1QiLCJhbGci0iJSUzI1NiJ9.eyJyZWluZGVlclNuYWNrIjoiY2Fycm90cyIsInNhbnRhSGF0Q29sb3IiOiJyZWQiLCJzbm93R2xvYmVDb2xsZWN0aW9uIjo1LCJzdG9ja2luZ1N0dWZmZXJzIjpbInlvLXlvIiwiY2FuZHkiLCJrZXljaGFpbiJdLCJ0cmVlSGVpZ2h0Ijo3fQ.EoWSlwZIMHdtd96U_FkfQ9SkbzskSvgEaRpsUeZQFJixDW57vZud_k-MK1R1LEGoJRPGttJvG_5ewdK9O46OuaGW4DHIOWIFLxSYFTJBdFMVmAWC6snqartAFr2U-LWxTwJ09WNpPBcL67YCx4HQsoGZ2mxRVNIKxR7IEfkZDhmpDkiAUbtKyn0H1EVERP1gdbzHUGpLd7wiuzkJnjenBgLPifUevxGPgj535cp8I6EeE4gLdMEm3lbUW4wX_GG5t6_fDAF4URfiAOkSbiIW6lKcSGD9MBVEGps88lA2REBEjT4c7XHw4Tbxci2-knuJm90zIA9KX92t96tF3VFKEA", 
        StatusCode::BAD_REQUEST, None)]
    #[case(
        "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.eyJnaWZ0cyI6WyJDb2FsIl19.DaVXV_czINRO1Cvhw33YSPSpV7_TYTqp7gIB_XiVl5fh3K9zkmDItBFLxJHyb7TRw_CGrAYwfinxn6_Dn9MMhp8d3tc-UnRskOxNHpqwU9EcbDtn31uHStT5sLfzdK0fdAc1XUJnr-9dbiGiYARO9YK7HAijdR8bCRMtvMUgIHsumWHO5BEE4CCeVgypzkebsoaev495OE0VNCfn1rSbTKR12xiIFoPCZALV9_slqoZvO59K0x8DSppx7uHApGjXvS6JmyjVgMJNuJoPrIYzc0nytVCa5uLjYIadS2inw7Sty1Jj-sLi8AgtYCXcpyB59MUXNP5xze_Sat8hmQ_NzQ", 
        StatusCode::UNAUTHORIZED, None)]
    #[test_log::test(tokio::test)]
    async fn test_decode(
        server: TestServer,
        #[case] jwt: &str,
        #[case] status: StatusCode,
        #[case] result: Option<&str>,
    ) {
        let response = server.post("/decode").text(jwt).await;

        response.assert_status(status);
        if let Some(result) = result {
            response.assert_json(&serde_json::from_str::<Value>(result).unwrap());
        }
    }
}
