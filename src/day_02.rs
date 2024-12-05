use std::net::{Ipv4Addr, Ipv6Addr};

use axum::{extract::Query, routing::get, Router};
use itertools::Itertools;
use serde::Deserialize;
use tracing::{debug, event, instrument, Level};

#[derive(Deserialize, Debug)]
struct EncryptionRequest {
    from: Box<str>,
    key: Box<str>,
}

#[derive(Deserialize, Debug)]
struct KeyRequest {
    from: Box<str>,
    to: Box<str>,
}

#[instrument]
async fn encryption(query: Query<EncryptionRequest>) -> String {
    let from = query.from.parse::<Ipv4Addr>().unwrap();
    let key = query.key.parse::<Ipv4Addr>().unwrap();

    let encrypted: (u8, u8, u8, u8) = from
        .octets()
        .iter()
        .zip(key.octets().iter())
        .map(|(from, key)| {
            event!(Level::DEBUG, ?from, ?key);

            from.wrapping_add(*key)
        })
        .collect_tuple()
        .unwrap();

    event!(Level::DEBUG, ?encrypted);

    Ipv4Addr::new(encrypted.0, encrypted.1, encrypted.2, encrypted.3).to_string()
}

#[instrument]
async fn key(query: Query<KeyRequest>) -> String {
    let from = query.from.parse::<Ipv4Addr>().unwrap();
    let to = query.to.parse::<Ipv4Addr>().unwrap();

    let key: (u8, u8, u8, u8) = from
        .octets()
        .iter()
        .zip(to.octets().iter())
        .map(|(from, to)| {
            event!(Level::DEBUG, ?from, ?to);

            to.wrapping_sub(*from)
        })
        .collect_tuple()
        .unwrap();

    event!(Level::DEBUG, ?key);

    Ipv4Addr::new(key.0, key.1, key.2, key.3).to_string()
}

#[instrument]
async fn v6_encryption(query: Query<EncryptionRequest>) -> String {
    let from = query.from.parse::<Ipv6Addr>().unwrap();
    let key = query.key.parse::<Ipv6Addr>().unwrap();

    let encrypted: (u16, u16, u16, u16, u16, u16, u16, u16) = from
        .segments()
        .iter()
        .zip(key.segments().iter())
        .map(|(from, key)| {
            event!(Level::DEBUG, ?from, ?key);

            from ^ key
        })
        .collect_tuple()
        .unwrap();

    event!(Level::DEBUG, ?encrypted);

    Ipv6Addr::new(
        encrypted.0,
        encrypted.1,
        encrypted.2,
        encrypted.3,
        encrypted.4,
        encrypted.5,
        encrypted.6,
        encrypted.7,
    )
    .to_string()
}

#[instrument]
async fn v6_key(query: Query<KeyRequest>) -> String {
    let from = query.from.parse::<Ipv6Addr>().unwrap();
    let to = query.to.parse::<Ipv6Addr>().unwrap();

    let encrypted: (u16, u16, u16, u16, u16, u16, u16, u16) = from
        .segments()
        .iter()
        .zip(to.segments().iter())
        .map(|(from, to)| {
            event!(Level::DEBUG, ?from, ?to);

            from ^ to
        })
        .collect_tuple()
        .unwrap();

    event!(Level::DEBUG, ?encrypted);

    Ipv6Addr::new(
        encrypted.0,
        encrypted.1,
        encrypted.2,
        encrypted.3,
        encrypted.4,
        encrypted.5,
        encrypted.6,
        encrypted.7,
    )
    .to_string()
}
#[instrument]
pub fn router() -> Router {
    debug!("Loading two routes");

    let v6_routes = Router::new()
        .route("/dest", get(v6_encryption))
        .route("/key", get(v6_key));

    Router::new()
        .route("/dest", get(encryption))
        .route("/key", get(key))
        .nest("/v6", v6_routes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case("10.0.0.0", "1.2.3.255", "11.2.3.255")]
    #[case("128.128.33.0", "255.0.255.33", "127.128.32.33")]
    #[test_log::test(tokio::test)]
    async fn test_encryption(#[case] from: &str, #[case] key: &str, #[case] expected: &str) {
        let query = Query(EncryptionRequest {
            from: from.into(),
            key: key.into(),
        });
        let result = encryption(query).await;
        assert_eq!(expected, result)
    }

    #[rstest::rstest]
    #[case("10.0.0.0", "11.2.3.255", "1.2.3.255")]
    #[case("128.128.33.0", "127.128.32.33", "255.0.255.33")]
    #[test_log::test(tokio::test)]
    async fn test_key(#[case] from: &str, #[case] to: &str, #[case] expected: &str) {
        let query = Query(KeyRequest {
            from: from.into(),
            to: to.into(),
        });
        let result = key(query).await;
        assert_eq!(expected, result)
    }

    #[rstest::rstest]
    #[case("fe80::1", "5:6:7::3333", "fe85:6:7::3332")]
    #[case("aaaa::aaaa", "ffff:ffff:c::c:1234:ffff", "5555:ffff:c::c:1234:5555")]
    #[test_log::test(tokio::test)]
    async fn test_v6_encryption(#[case] from: &str, #[case] key: &str, #[case] expected: &str) {
        let query = Query(EncryptionRequest {
            from: from.into(),
            key: key.into(),
        });
        let result = v6_encryption(query).await;
        assert_eq!(expected, result)
    }

    #[rstest::rstest]
    #[case(
        "aaaa::aaaa",
        "5555:ffff:c:0:0:c:1234:5555",
        "ffff:ffff:c::c:1234:ffff"
    )]
    #[case("fe80::1", "fe85:6:7::3332", "5:6:7::3333")]
    #[case(
        "feed:beef:deaf:bad:cafe::",
        "feed:beef:deaf:bad:c755:bed:ace:dad",
        "::dab:bed:ace:dad"
    )]
    #[test_log::test(tokio::test)]
    async fn test_v6_key(#[case] from: &str, #[case] to: &str, #[case] expected: &str) {
        let query = Query(KeyRequest {
            from: from.into(),
            to: to.into(),
        });
        let result = v6_key(query).await;
        assert_eq!(expected, result)
    }
}
