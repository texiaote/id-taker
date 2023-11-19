use std::sync::Arc;

use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tokio::sync::Mutex;

use crate::db;
use crate::id_generator::IdGenerator;
use crate::models::IdSegments;

#[derive(Deserialize)]
pub struct CreateIdRequest {
    biz_tag: String,
}

#[derive(Serialize)]
pub struct CreateIdResponse {
    success: bool,
    message: String,
}

#[derive(Deserialize)]
pub struct GetIdRequest {
    biz_tag: String,
}

#[derive(Serialize)]
pub struct GetIdResponse {
    id: i64,
}

pub async fn create_id(Extension(pool): Extension<Arc<MySqlPool>>, Json(request): Json<CreateIdRequest>) -> Json<CreateIdResponse> {
    let segment = IdSegments::new(&request.biz_tag);
    let result = db::create_id_segment(&pool, &segment).await;

    match result {
        Ok(_) => Json(CreateIdResponse {
            success: true,
            message: "Id segment created successfully.".into(),
        }),
        Err(e) => Json(CreateIdResponse {
            success: false,
            message: format!("Error: {}", e),
        })
    }
}

pub async fn get_id(Extension(pool): Extension<Arc<Mutex<IdGenerator>>>, Json(request): Json<GetIdRequest>) -> Json<GetIdResponse> {
    let mut id_generator = pool.lock().await;
    match id_generator.get_id(&request.biz_tag).await {
        Ok(id) => Json(GetIdResponse { id }),
        Err(_) => Json(GetIdResponse { id: -1 })
    }
}