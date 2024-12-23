#![allow(dead_code)]

use axum::{
    extract::{Multipart, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use tracing::{debug, info};

async fn star() -> Response {
    debug!("Calling star");
    r#"<div id="star" class="lit"></div>"#.into_response()
}

async fn present(Path(color): Path<String>) -> Response {
    debug!("Calling present:{}", color);

    match color.as_str() {
        "red" => r#"
<div class="present red" hx-get="/23/present/blue" hx-swap="outerHTML">
    <div class="ribbon"></div>
    <div class="ribbon"></div>
    <div class="ribbon"></div>
    <div class="ribbon"></div>
</div>
        "#
        .into_response(),
        "blue" => r#"
<div class="present blue" hx-get="/23/present/purple" hx-swap="outerHTML">
    <div class="ribbon"></div>
    <div class="ribbon"></div>
    <div class="ribbon"></div>
    <div class="ribbon"></div>
</div>
        "#
        .into_response(),
        "purple" => r#"
<div class="present purple" hx-get="/23/present/red" hx-swap="outerHTML">
    <div class="ribbon"></div>
    <div class="ribbon"></div>
    <div class="ribbon"></div>
    <div class="ribbon"></div>
</div>
        "#
        .into_response(),
        _ => StatusCode::IM_A_TEAPOT.into_response(),
    }
}

async fn ornament(Path((state, n)): Path<(String, String)>) -> Response {
    debug!("Calling ornament:{}:{}", state, n);

    // let Ok(n) = n.parse::<u8>() else {
    //     return StatusCode::IM_A_TEAPOT.into_response();
    // };

    // if n < 1 || n > 7 {
    //     return StatusCode::IM_A_TEAPOT.into_response();
    // }
    let n = tera::escape_html(n.as_str());

    match state.as_str() {
        "on" => format!(r#"<div class="ornament on" id="ornament{n}" hx-trigger="load delay:2s once" hx-get="/23/ornament/off/{n}" hx-swap="outerHTML"></div>"#).into_response(),
        "off" => format!(r#"<div class="ornament" id="ornament{n}" hx-trigger="load delay:2s once" hx-get="/23/ornament/on/{n}" hx-swap="outerHTML"></div>"#).into_response(),
        _ => StatusCode::IM_A_TEAPOT.into_response(),
    }
}

#[derive(Deserialize, Debug)]
struct Lockfile {
    package: Vec<Package>,
}

#[derive(Deserialize, Debug)]
struct Package {
    checksum: Option<String>,
}

async fn lockfile(mut multipart: Multipart) -> Response {
    let mut file = None;
    while let Some(field) = multipart.next_field().await.unwrap_or_default() {
        let name = field.name().unwrap_or_default().to_string();
        let data = field.bytes().await.unwrap_or_default();

        info!(?name);
        if name.as_str() == "lockfile" {
            file = String::from_utf8(data.to_vec()).ok()
        }
    }

    let lockfile: Result<Lockfile, _> = toml::from_str(&file.unwrap_or_default());

    info!(?lockfile);
    if let Ok(lockfile) = lockfile {
        info!(?lockfile);
        let result = lockfile
            .package
            .iter()
            .filter_map(|p| p.checksum.as_ref())
            .map(|checksum| {

                if checksum.len() < 10 {
                    return Err("Not enough characters");
                }

                let checksum = checksum.to_lowercase();
                if checksum.chars().any(|c| matches!(c, 'g'..='z')) {
                    return Err("Invalid characters");
                }

                let color = &checksum[..6];
                let top = u64::from_str_radix(&checksum[6..8], 16);
                let left = u64::from_str_radix(&checksum[8..10], 16);

                let topleft = top.ok().zip(left.ok());

                if let Some((top, left)) = topleft {
                    Ok(format!(r#"<div style="background-color:#{color};top:{top}px;left:{left}px;"></div>"#))
                } else {
                    Err("failed to decode top/left")
                }
            })
            .collect::<Result<Vec<_>, &str>>();
        
        info!(?result);
        if let Ok(result) = result {
            result.join("\n").into_response()
        } else {
            StatusCode::UNPROCESSABLE_ENTITY.into_response()
        }
    } else {
        StatusCode::BAD_REQUEST.into_response()
    }
}

pub fn router() -> Router {
    debug!("Loading routes");
    Router::new()
        .route("/star", get(star))
        .route("/present/:color", get(present))
        .route("/ornament/:sate/:n", get(ornament))
        .route("/lockfile", post(lockfile))
}
