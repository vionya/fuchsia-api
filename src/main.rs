use fuchsia_api::{circlize_img, resize_img, security::CheckOrigin};
use image::{ImageError, ImageFormat};

use actix_multipart::Multipart;
use actix_web::{
    http::{Error, StatusCode},
    middleware, post, web, App, HttpResponse, HttpServer,
};
use futures::{future, FutureExt, StreamExt, TryStreamExt};
use serde::Deserialize;

const MAX_SIZE: usize = 2_000_000;

async fn load_data(buf: &mut Vec<u8>, mut payload: Multipart) -> Option<HttpResponse> {
    let mut size = 0;
    while let Ok(Some(mut field)) = payload.try_next().await {
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            size += data.len();
            buf.extend(data);

            if size > MAX_SIZE {
                return Some(HttpResponse::PayloadTooLarge().body(format!(
                    "Cannot upload more than {} bytes at once",
                    MAX_SIZE
                )));
            }
        }
    }
    None
}

const fn get_frames_default() -> usize {
    250
}

const fn get_aspect_default() -> bool {
    false
}

#[derive(Deserialize)]
struct ResizeInfo {
    width: u32,
    height: u32,
    #[serde(default = "get_frames_default")]
    frames: usize,
    #[serde(default = "get_aspect_default")]
    keep_aspect: bool,
}
#[post("/actions/resize")]
async fn resize(info: web::Query<ResizeInfo>, payload: Multipart) -> Result<HttpResponse, Error> {
    let mut all_data: Vec<u8> = Vec::new();
    if let Some(resp) = load_data(&mut all_data, payload).await {
        return Ok(resp);
    };

    web::block(move || {
        resize_img(
            &all_data,
            info.width,
            info.height,
            info.frames,
            info.keep_aspect,
        )
    })
    .then(|res| match res {
        Ok(Ok((bytes, fmt, (width, height)))) => future::ok(
            HttpResponse::build(StatusCode::OK)
                .content_type(if fmt == ImageFormat::Gif {
                    "image/gif"
                } else {
                    "image/png"
                })
                .append_header(("X-Width", width))
                .append_header(("X-Height", height))
                .body(bytes),
        ),
        Ok(Err(ImageError::Unsupported(_))) => future::ok(
            HttpResponse::ServiceUnavailable()
                .body("Only GIF, PNG, JPEG, and WEBP images are supported"),
        ),
        _ => future::ok(
            HttpResponse::ServiceUnavailable()
                .body("Something went wrong when trying to resize the image, sorry!"),
        ),
    })
    .await
}

#[derive(Deserialize)]
struct CirclizeInfo {
    dim: u32,
    #[serde(default = "get_frames_default")]
    frames: usize,
}
#[post("/actions/circlize")]
async fn circlize(
    info: web::Query<CirclizeInfo>,
    payload: Multipart,
) -> Result<HttpResponse, Error> {
    let mut all_data: Vec<u8> = Vec::new();
    if let Some(resp) = load_data(&mut all_data, payload).await {
        return Ok(resp);
    };

    web::block(move || circlize_img(&all_data, info.dim, info.frames))
        .then(|res| match res {
            Ok(Ok((bytes, fmt, _))) => future::ok(
                HttpResponse::build(StatusCode::OK)
                    .content_type(if fmt == ImageFormat::Gif {
                        "image/gif"
                    } else {
                        "image/png"
                    })
                    .body(bytes),
            ),
            Ok(Err(ImageError::Unsupported(_))) => future::ok(
                HttpResponse::ServiceUnavailable()
                    .body("Only GIF, PNG, JPEG, and WEBP images are supported"),
            ),
            _ => future::ok(
                HttpResponse::ServiceUnavailable()
                    .body("Something went wrong when trying to resize the image, sorry!"),
            ),
        })
        .await
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();

    let host = std::env::args()
        .skip(1)
        .next()
        .expect("A host must be provided to argv");
    let port = std::env::args()
        .skip(2)
        .next()
        .expect("A port must be provided to argv");

    let h = host.clone();
    HttpServer::new(move || {
        let h = h.to_string();

        App::new()
            .wrap(CheckOrigin::new(h))
            .wrap(middleware::Logger::default())
            .service(resize)
            .service(circlize)
    })
    .bind(format!("{}:{}", &host, &port))?
    .run()
    .await
}
