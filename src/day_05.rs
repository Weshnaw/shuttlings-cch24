#![allow(dead_code)]

use std::str::FromStr;

use axum::{
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use cargo_manifest::Manifest;
use indexmap::IndexMap;
use serde::Deserialize;
use toml::Value;
use tracing::{debug, info, instrument, warn};

#[derive(Deserialize, Debug, Default)]
struct Metadata {
    orders: Vec<Value>,
}

#[derive(Deserialize, Debug)]
struct Order {
    item: String,
    quantity: u32,
}

async fn manifest(headers: HeaderMap, data: String) -> (StatusCode, String) {
    let content = headers.get("Content-Type");

    let cargo = match content
        .map(|header| header.to_str().unwrap_or_default())
        .unwrap_or_default()
    {
        "application/toml" => Manifest::from_str(&data).map_err(|_| "Invalid toml"),
        "application/json" => serde_json::from_str(&data).map_err(|_| "Invalid json"),
        "application/yaml" => serde_yaml::from_str(&data).map_err(|_| "Invalid yaml"),
        _ => return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "".to_string()),
    };

    info!(?data);
    if let Ok(cargo) = cargo {
        info!(?cargo);
        let mut orders = IndexMap::new();
        let (order_vec, keywords) = cargo.package.map_or((vec![], vec![]), |package| {
            let metadata = package.metadata.map(|metadata| {
                let metadata: Metadata = metadata.try_into().unwrap_or_default();

                metadata
            });

            (
                metadata.unwrap_or_default().orders,
                package
                    .keywords
                    .map_or(vec![], |keywords| keywords.as_local().unwrap_or_default()),
            )
        });

        if !keywords.contains(&"Christmas 2024".to_string()) {
            return (
                StatusCode::BAD_REQUEST,
                "Magic keyword not provided".to_string(),
            );
        }

        order_vec.into_iter().for_each(|order| {
            let order: Result<Order, _> = order.try_into();
            if let Ok(order) = order {
                debug!(?order);
                let current_count: &mut u32 = orders.entry(order.item).or_default();
                *current_count += order.quantity;
            }
        });

        info!(?orders);
        if orders.len() > 0 {
            let result = orders
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n");
            info!(?result);
            (StatusCode::OK, result)
        } else {
            (StatusCode::NO_CONTENT, "".to_string())
        }
    } else {
        warn!("Bad manifest");
        (StatusCode::BAD_REQUEST, "Invalid manifest".to_string())
    }
}

#[instrument]
pub fn router() -> Router {
    debug!("Loading routes");
    Router::new().route("/manifest", post(manifest))
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_valid_manifest() {
        let data = r#"
[package]
name = "not-a-gift-order"
authors = ["Not Santa"]
keywords = ["Christmas 2024"]

[[package.metadata.orders]]
item = "Toy car"
quantity = 2

[[package.metadata.orders]]
item = "Lego brick"
quantity = 230
"#;
        let expected = r#"Toy car: 2
Lego brick: 230"#;

        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Type",
            HeaderValue::from_str("application/toml").unwrap(),
        );

        let (status, body) = manifest(headers, data.to_string()).await;

        assert_eq!(StatusCode::OK, status);
        assert_eq!(expected, body);
    }

    #[test_log::test(tokio::test)]
    async fn test_no_orders() {
        let data = r#"
[package]
name = "coal-in-a-bowl"
authors = ["H4CK3R_13E7"]
keywords = ["Christmas 2024"]

[[package.metadata.orders]]
item = "Coal"
quantity = "Hahaha get rekt"
"#;
        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Type",
            HeaderValue::from_str("application/toml").unwrap(),
        );

        let (status, body) = manifest(headers, data.to_string()).await;

        assert_eq!(StatusCode::from_u16(204).unwrap(), status);
        assert_eq!("", body);
    }

    #[test_log::test(tokio::test)]
    async fn test_invalid_manifest() {
        let data = r#"
[package]
name = false
authors = ["Not Santa"]
keywords = ["Christmas 2024"]
"#;
        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Type",
            HeaderValue::from_str("application/toml").unwrap(),
        );

        let (status, body) = manifest(headers, data.to_string()).await;

        assert_eq!(StatusCode::from_u16(400).unwrap(), status);
        assert_eq!("Invalid manifest", body);
    }

    #[test_log::test(tokio::test)]
    async fn test_invalid_keyword() {
        let data = r#"
[package]
name = "grass"
authors = ["Not Santa"]
keywords = ["Mooooo"]
"#;
        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Type",
            HeaderValue::from_str("application/toml").unwrap(),
        );

        let (status, body) = manifest(headers, data.to_string()).await;

        assert_eq!(StatusCode::from_u16(400).unwrap(), status);
        assert_eq!("Magic keyword not provided", body);
    }

    // TODO: yaml, json, invalid content-type unit tests
}
