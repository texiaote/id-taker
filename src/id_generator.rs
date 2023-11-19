use std::collections::HashMap;

use sqlx::MySqlPool;

use crate::db;

pub struct IdSegment {
    pub max_id: i64,
    pub step: i64,
    pub next_id: i64,
}

impl IdSegment {
    fn new(next_id: i64, step: i64) -> Self {
        Self {
            next_id,
            step,
            max_id: next_id + step,

        }
    }
}

pub struct IdGenerator {
    buffer: HashMap<String, IdSegment>,
    db_pool: MySqlPool,
}

impl IdGenerator {
    pub fn new(db_pool: MySqlPool) -> Self {
        Self {
            buffer: HashMap::new(),
            db_pool,
        }
    }

    pub async fn get_id(&mut self, biz_tag: &str) -> Result<i64, sqlx::Error> {
        if let Some(segment) = self.buffer.get_mut(biz_tag) {
            if segment.next_id < segment.max_id {
                let id = segment.next_id;
                // 自动加1
                segment.next_id += 1;
                return Ok(id);
            }
        }

        self.load_new_segment(biz_tag).await?;

        if let Some(segment) = self.buffer.get_mut(biz_tag) {
            let id = segment.next_id;
            segment.next_id += 1;
            return Ok(id);
        }
        Err(sqlx::Error::RowNotFound)
    }

    async fn load_new_segment(&mut self, biz_tag: &str) -> Result<(), sqlx::Error> {
        let new_segment = db::fetch_new_segment(&self.db_pool, biz_tag).await?;

        let segment = IdSegment::new(new_segment.max_id, new_segment.step);
        self.buffer.insert(biz_tag.to_string(), segment);

        Ok(())
    }
}