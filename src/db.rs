use sqlx::MySqlPool;
use crate::id_generator::IdSegment;
use crate::models::IdSegments;

pub async fn create_id_segment(pool: &MySqlPool, new_segment: &IdSegments) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO id_segments (biz_tag, max_id, step) VALUES (?, ?, ?)")
        .bind(&new_segment.biz_tag)
        .bind(new_segment.max_id)
        .bind(new_segment.step)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_id_segment(pool: &MySqlPool, biz_tag: &str) -> Result<IdSegments, sqlx::Error> {
    let segment = sqlx::query_as::<_, IdSegments>("SELECT biz_tag, max_id, step FROM id_segments WHERE biz_tag = ?").bind(biz_tag).fetch_one(pool).await?;

    Ok(segment)
}

pub async fn fetch_new_segment(pool: &MySqlPool, biz_tag: &str) -> Result<IdSegments, sqlx::Error> {
    let mut tx = pool.begin().await?;
    // 查询当前ID段信息
    let mut segment = sqlx::query_as::<_, IdSegments>(
        "SELECT biz_tag, max_id, step FROM id_segments WHERE biz_tag = ? FOR UPDATE")
        .bind(biz_tag)
        .fetch_one(&mut *tx)
        .await?;

    // 计算新的最大ID
    let new_max_id = segment.max_id + segment.step;

    // 更新数据库中的ID段信息
    sqlx::query("UPDATE id_segments SET max_id = ? WHERE biz_tag = ?")
        .bind(new_max_id)
        .bind(biz_tag)
        .execute(&mut *tx)
        .await?;

    // 提交事务
    tx.commit().await?;

    // 更新内存中的ID段对象
    segment.max_id = new_max_id;

    Ok(segment)
}