use std::cmp;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use sqlx::MySqlPool;
use tokio::sync::Mutex;

use crate::db;

pub struct IdSegment {
    pub max_id: u64,
    pub step: u64,
    pub next_id: u64,
}

pub struct SegmentBuffer {
    biz_tag: String,
    buffer: [Arc<Mutex<IdSegment>>; 2],
    index: AtomicUsize,
    db_pool: MySqlPool,
}

impl SegmentBuffer {
    pub fn new(biz_tag: String, db_pool: MySqlPool, id_segment: IdSegment) -> Self {
        Self {
            biz_tag,
            buffer: [Arc::new(Mutex::new(id_segment)), Arc::new(Mutex::new(IdSegment::new(0, 0)))],
            index: AtomicUsize::new(0),
            db_pool,
        }
    }

    pub async fn get_next_id(&mut self, count: Option<usize>) -> Result<Vec<u64>, sqlx::Error> {
        let count = count.unwrap_or(1);

        let mut vec = vec![];

        loop {
            let remaining = (count - vec.len()) as u64;

            let index = self.index.load(Ordering::SeqCst);

            let mut segment = self.buffer[index].lock().await;

            if segment.step > 0 && segment.next_id < segment.max_id {
                let id = segment.next_id;

                let max = cmp::min(segment.next_id + remaining, segment.max_id);

                let slices: Vec<u64> = (id..max).collect();
                let _ = vec.extend(&slices);
                segment.next_id += remaining;
            }

            if vec.len() == count {
                break;
            }

            let buffer = self.buffer[index].clone();

            // 说明该号段的数据已经用完了，需要切换新的号段
            self.index = AtomicUsize::new((index + 1) % 2);

            // Exchange Success, the old buffer load segments async in background
            tokio::spawn(load_new_segment(self.db_pool.clone(), self.biz_tag.to_string(), buffer));
        }
        Ok(vec)
    }
}

async fn load_new_segment(pool: MySqlPool, biz_tag: String, buffer: Arc<Mutex<IdSegment>>) -> Result<(), sqlx::Error> {
    let new_segment = db::fetch_new_segment(&pool, &biz_tag).await?;

    let mut buffer = buffer.lock().await;

    buffer.next_id = new_segment.max_id;
    buffer.step = new_segment.step;
    buffer.max_id = new_segment.max_id + new_segment.step;

    Ok(())
}

impl IdSegment {
    fn new(initial_id: u64, step: u64) -> Self {
        Self {
            next_id: initial_id,
            step,
            max_id: initial_id + step,
        }
    }
}

pub struct IdGenerator {
    map: HashMap<String, SegmentBuffer>,
    db_pool: MySqlPool,
}

impl IdGenerator {
    pub fn new(db_pool: MySqlPool) -> Self {
        Self {
            map: HashMap::new(),
            db_pool,
        }
    }

    pub async fn get_id(&mut self, biz_tag: &str, count: Option<usize>) -> Result<Vec<u64>, sqlx::Error> {

        // 如果没有，则尝试从数据库中获取
        if self.map.get(biz_tag).is_none() {

            // 查询数据库，查看有没有
            let id_segment = db::get_id_segment(&self.db_pool, biz_tag).await?;

            let segment = IdSegment::new(id_segment.max_id, id_segment.step);

            let segment_buffer = SegmentBuffer::new(biz_tag.to_string(), self.db_pool.clone(), segment);
            self.map.insert(biz_tag.to_string(), segment_buffer);
        }

        if let Some(segment_buffer) = self.map.get_mut(biz_tag) {
            return segment_buffer.get_next_id(count).await;
        }

        Ok(vec![])
    }
}

