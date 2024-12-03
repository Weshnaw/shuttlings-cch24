use axum::{extract::Query, routing::get, Router};
use serde::Deserialize;
use tracing::{debug, event, instrument, Level};
use itertools::Itertools;

#[derive(Deserialize, Debug)]
struct EncryptionRequest {
    from: Box<str>,
    key: Box<str>
}

#[derive(Deserialize, Debug)]
struct KeyRequest {
    from: Box<str>,
    to: Box<str>
}

#[instrument]
async fn encryption(query: Query<EncryptionRequest>) -> String {
    let from = query.from.split('.').into_iter();
    let key = query.key.split('.').into_iter();

    let encrypted = from.zip(key).map(|(from, key)| {
        let (from, key) = match (from.parse::<u8>(), key.parse::<u8>()) {
            (Ok(from), Ok(key)) => (from, key),
            _ => {
                panic!("Failed to parse numbers {from}, {key}")
            }
        };

        from.wrapping_add(key)
    }).join(".");
    
    encrypted
}

#[instrument]
async fn key(query: Query<KeyRequest>) -> String {
    let from = query.from.split('.').into_iter();
    let to = query.to.split('.').into_iter();

    let key = from.zip(to).map(|(from, to)| {
        let (from, to) = match (from.parse::<u8>(), to.parse::<u8>()) {
            (Ok(from), Ok(to)) => (from, to),
            _ => {
                panic!("Failed to parse numbers {from}, {to}")
            }
        };

        to.wrapping_sub(from)
    }).join(".");
    
    key
}

#[instrument]
fn unshorten_v6(ipv6: &str) -> String {
    debug!("unshortening ipv6");
    // 7 ':' should exist
    let splits = ipv6.chars().filter(|c| *c == ':').count();
    match ipv6.split("::").collect::<Vec<_>>().as_slice() {
        [left, right] => {
            // should give the total number of ':' minut the two used for shortening
            let remaining_count = splits - 2;

            // Take the unshortened number of ':' subtract out the ones that still exist in string
            // and fill in that number of ':'
            let unshortened = ":".repeat(7 - remaining_count);
            [left.to_string(), unshortened, right.to_string()].join("")
        },
        _ => {
            ipv6.into()
        }
    }
}

#[instrument]
fn shorten_v6(ipv6: &str) -> String {
    debug!("shortening ipv6");
    let zero_idx = ipv6.find(":0:");
    
    match zero_idx {
        Some(idx) => {
            let mut shortened = ipv6.split(":").into_iter().filter(|part| *part != "0").join(":");

            shortened.insert(idx, ':');

            shortened
        }, 
        None => ipv6.into()
    }
}

#[instrument]
async fn v6_encryption(query: Query<EncryptionRequest>) -> String {
    let from = unshorten_v6(&query.from);
    let key = unshorten_v6(&query.key);

    let from = from.split(':').into_iter();
    let key = key.split(':').into_iter();

    let encrypted = from.zip(key).map(|(from, key)| {
        let from = format!("{:0>4}", from);
        let key = format!("{:0>4}", key);
        let (from, key) = match (u16::from_str_radix(&from, 16), u16::from_str_radix(&key, 16)) {
            (Ok(from), Ok(key)) => (from, key),
            _ => {
                panic!("Failed to parse numbers {from}, {key}")
            }
        };

        event!(Level::DEBUG, ?from, ?key);

        format!("{:x}", from ^ key)
    }).join(":");
    
    event!(Level::DEBUG, ?encrypted);
    shorten_v6(&encrypted)
}

#[instrument]
async fn v6_key(query: Query<KeyRequest>) -> String {
    let from = unshorten_v6(&query.from);
    let to = unshorten_v6(&query.to);

    let from = from.split(':').into_iter();
    let to = to.split(':').into_iter();

    let key = from.zip(to).map(|(from, to)| {
        let from = format!("{:0>4}", from);
        let to = format!("{:0>4}", to);
        let (from, to) = match (u16::from_str_radix(&from, 16), u16::from_str_radix(&to, 16)) {
            (Ok(from), Ok(to)) => (from, to),
            _ => {
                panic!("Failed to parse numbers {from}, {to}")
            }
        };

        event!(Level::DEBUG, ?from, ?to);
        format!("{:x}", from ^ to)
    }).join(":");
    
    event!(Level::DEBUG, ?key);
    shorten_v6(&key)
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
            from: from.into(), key: key.into()
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
            from: from.into(), to: to.into()
        });
        let result = key(query).await;
        assert_eq!(expected, result)
    }

    #[rstest::rstest]
    #[case("fe80::1", "fe80:::::::1")]
    #[case("5:6:7::3333", "5:6:7:::::3333")]
    #[case("fe85:6:7::3332", "fe85:6:7:::::3332")]
    #[case("aaaa::aaaa", "aaaa:::::::aaaa")]
    #[case("ffff:ffff:c::c:1234:ffff", "ffff:ffff:c:::c:1234:ffff")]
    #[case("5555:ffff:c::c:1234:5555", "5555:ffff:c:::c:1234:5555")]
    #[case("a:a:a:a:a:a:a:a", "a:a:a:a:a:a:a:a")]
    #[case("a::a", "a:::::::a")]
    #[test_log::test]
    fn test_unshorten_v6(#[case] ipv6: &str, #[case] expected: &str) {
        let result = unshorten_v6(ipv6);
        assert_eq!(expected, result)
    }

    #[rstest::rstest]
    #[case("fe80:0:0:0:0:0:0:1", "fe80::1")]
    #[case("a:a:a:a:a:a:0:a", "a:a:a:a:a:a::a")]
    #[test_log::test]
    fn test_shorten_v6(#[case] ipv6: &str, #[case] expected: &str) {
        let result = shorten_v6(ipv6);
        assert_eq!(expected, result)
    }


    #[rstest::rstest]
    #[case("fe80::1", "5:6:7::3333", "fe85:6:7::3332")]
    #[case("aaaa::aaaa", "ffff:ffff:c::c:1234:ffff", "5555:ffff:c::c:1234:5555")]
    #[test_log::test(tokio::test)]
    async fn test_v6_encryption(#[case] from: &str, #[case] key: &str, #[case] expected: &str) {
        let query = Query(EncryptionRequest {
            from: from.into(), key: key.into()
        });
        let result = v6_encryption(query).await;
        assert_eq!(expected, result)
    }

    #[rstest::rstest]
    #[case("aaaa::aaaa", "5555:ffff:c:0:0:c:1234:5555", "ffff:ffff:c::c:1234:ffff")]
    #[case("fe80::1", "fe85:6:7::3332", "5:6:7::3333")]
    #[test_log::test(tokio::test)]
    async fn test_v6_key(#[case] from: &str, #[case] to: &str, #[case] expected: &str) {
        let query = Query(KeyRequest {
            from: from.into(), to: to.into()
        });
        let result = v6_key(query).await;
        assert_eq!(expected, result)
    }
}
