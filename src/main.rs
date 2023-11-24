use std::net::SocketAddr;
use std::sync::{Arc};

use axum::Router;
use axum::routing::{get, post};
use sqlx::MySqlPool;
use tokio::sync::Mutex;

use crate::id_generator::IdGenerator;

mod config;
mod db;
mod models;
mod routes;
mod id_generator;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let config = config::load_config().expect("Failed to load configuration");
    let pool = MySqlPool::connect(&config.database_url).await.expect("Failed to connect the database");

    let id_generator = Arc::new(Mutex::new(IdGenerator::new(pool.clone())));

    let app = Router::new()
        .route("/create_biz_tag", post(routes::create_biz_tag))
        .route("/get_id", get(routes::get_id))
        .route("/batch_get_ids", get(routes::batch_get_id))
        .layer(axum::extract::Extension(id_generator))
        .layer(axum::extract::Extension(Arc::new(pool)));

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    println!("Listening on {}", addr);

    axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
}



