use std::cmp;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use snowflake_rs::{SnowFlakeId, STANDARD_EPOCH};

use sqlx::MySqlPool;
use tokio::sync::RwLock;

use crate::db;

pub struct IdSegment {
    pub max_id: u64,
    pub step: u64,
    pub next_id: AtomicU64,
    pub snowflake: SnowFlakeId,
}

impl IdSegment {
    fn new(next_id: u64, step: u64) -> Self {
        Self {
            next_id: AtomicU64::new(next_id),
            step,
            max_id: next_id + step,
            snowflake: SnowFlakeId::new(1, STANDARD_EPOCH),
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

    pub async fn get_id(&self, biz_tag: &str, count: Option<usize>) -> Result<Vec<u64>, sqlx::Error> {

        // 至少是1条数据
        let count = count.unwrap_or(1);

        let mut vec = vec![];
        loop {
            // 找到剩余的数量
            let remaining = (count - vec.len()) as u64;
            let index = self.index.load(Ordering::SeqCst);
            let buffer = self.buffer[index].read().await;

            // 尝试从号段中获取数据
            if let Some(segment) = buffer.get(biz_tag) {

                // 判断是否要用雪花算法

                if segment.next_id.load(Ordering::SeqCst) < segment.max_id {

                    // 将数据都拿出去
                    let id = segment.next_id.load(Ordering::SeqCst);


                    let max = cmp::min(segment.next_id.load(Ordering::SeqCst) + remaining, segment.max_id);

                    // 从当前值，一直到max的值
                    let slices: Vec<u64> = (id..max).collect();
                    let _ = vec.extend(&slices);

                    // 更新字段
                    segment.next_id.fetch_add(remaining, Ordering::SeqCst);
                }

                // 还有数据，需要继续查找
                if vec.len() < count {

                    // 说明该号段的数据已经用完了，需要切换新的号段
                    match self.index.compare_exchange(index, (index + 1) % 2, Ordering::SeqCst, Ordering::SeqCst) {
                        Ok(_) => {
                            // Exchange Success, the old buffer load segments async in background
                            tokio::spawn(load_new_segment(self.db_pool.clone(), biz_tag.to_string(), self.buffer[index].clone()));
                        }
                        Err(e) => {
                            println!("切换失败,{}", e);
                        }
                    }
                    // 切换完成后，继续下一步的操作
                } else {
                    return Ok(vec);
                }
            } else {
                // 这个是释放读锁，不然会引起死锁

                drop(buffer);

                load_new_segment(self.db_pool.clone(), biz_tag.to_string(), self.buffer[index].clone()).await?;
            }
        }
    }
}

async fn load_new_segment(pool: MySqlPool, biz_tag: String, buffer: Arc<RwLock<HashMap<String, IdSegment>>>) -> Result<(), sqlx::Error> {
    let new_segment = db::fetch_new_segment(&pool, &biz_tag).await?;

    let segment = IdSegment::new(new_segment.max_id, new_segment.step);
    buffer.write().await.insert(biz_tag, segment);

    Ok(())
}