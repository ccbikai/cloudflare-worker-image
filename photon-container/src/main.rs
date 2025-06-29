use actix_web::{web, App, Error, HttpResponse, HttpServer, Responder};
use serde::Serialize;

mod actions;
mod encoder;
mod error;
mod processor;
mod utils;
use processor::{process_image, ImageQuery};

#[derive(Serialize)]
struct StatusResponse<'a> {
    status: &'a str,
    version: &'a str,
    name: &'a str,
    workers: usize,
    envs: std::collections::HashMap<String, String>,
}

async fn status() -> impl Responder {
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get());
    HttpResponse::Ok().json(StatusResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        name: env!("CARGO_PKG_NAME"),
        workers,
        envs: std::env::vars().collect(),
    })
}

async fn apply_photon_effect(query: web::Query<ImageQuery>) -> Result<HttpResponse, Error> {
    let (buf, content_type) = process_image(query).await?;

    // Return the new image with a one-year cache header
    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .append_header(("Cache-Control", "public, max-age=31536000"))
        .body(buf))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get());

    println!(
        "Photon-Container server starting on http://0.0.0.0:8000 with {} worker(s)",
        workers
    );

    HttpServer::new(|| {
        App::new()
            .route("/status", web::get().to(status))
            .route("/", web::get().to(apply_photon_effect))
    })
    .workers(workers)
    .bind("0.0.0.0:8000")?
    .run()
    .await
}
