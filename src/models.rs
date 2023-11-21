use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow)]
pub struct IdSegments {
    pub biz_tag: String,
    pub max_id: u64,
    pub step: u64,
}

impl IdSegments {
    pub fn new(biz_tag: &str, initial_id: u64, step: u64) -> Self {
        Self {
            biz_tag: biz_tag.to_string(),
            max_id: initial_id,
            step,
        }
    }
}