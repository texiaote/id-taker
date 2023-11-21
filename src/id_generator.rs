use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI16, AtomicI64, AtomicI8, AtomicUsize, Ordering};

use sqlx::MySqlPool;
use tokio::sync::{Mutex, RwLock};
use tokio::task_local;

use crate::db;

pub struct IdSegment {
    pub max_id: i64,
    pub step: i64,
    pub next_id: AtomicI64,
}

impl IdSegment {
    fn new(next_id: i64, step: i64) -> Self {
        Self {
            next_id: AtomicI64::new(next_id),
            step,
            max_id: next_id + step,

        }
    }
}

pub struct IdGenerator {
    buffer: [Arc<RwLock<HashMap<String, IdSegment>>>; 2],
    db_pool: MySqlPool,
    index: AtomicUsize,
}

impl IdGenerator {
    pub fn new(db_pool: MySqlPool) -> Self {
        Self {
            buffer: [Arc::new(RwLock::new(HashMap::new())), Arc::new(RwLock::new(HashMap::new()))],
            db_pool,
            index: AtomicUsize::new(0),
        }
    }

    pub async fn get_id(&self, biz_tag: &str) -> Result<i64, sqlx::Error> {
        loop {
            let index = self.index.load(Ordering::SeqCst);

            let buffer = self.buffer[index].clone();
            // 尝试从号段中获取数据
            if let Some(segment) = buffer.read().await.get(biz_tag) {
                if segment.next_id.load(Ordering::SeqCst) < segment.max_id {
                    let id = segment.next_id.fetch_add(1, Ordering::SeqCst);
                    return Ok(id);
                }
                // 说明该号段的数据已经用完了，需要切换新的号段
                match self.index.compare_exchange(index, (index + 1) % 2, Ordering::SeqCst, Ordering::SeqCst) {
                    Ok(_) => {

                        // Exchange Success, the old buffer load segments async in background
                        tokio::spawn(load_new_segment(self.db_pool.clone(), biz_tag.to_string(), buffer.clone()));
                    }
                    Err(e) => {
                        println!("切换失败,{}", e);
                    }
                }
                // 切换完成后，继续下一步的操作
                continue;
            }
            // 尝试从数据库中获取数据，如果还是没有，则报错
            load_new_segment(self.db_pool.clone(), biz_tag.to_string(), buffer.clone()).await?;
        }
    }
}

async fn load_new_segment(pool: MySqlPool, biz_tag: String, buffer: Arc<RwLock<HashMap<String, IdSegment>>>) -> Result<(), sqlx::Error> {
    let new_segment = db::fetch_new_segment(&pool, &biz_tag).await?;

    let segment = IdSegment::new(new_segment.max_id, new_segment.step);
    buffer.write().await.insert(biz_tag, segment);

    Ok(())
}