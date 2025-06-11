use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use crate::models::Meetup;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn hello() -> impl IntoResponse {
    (StatusCode::OK, "Hello, World!")
}

// 定义查询参数结构体
#[derive(Deserialize)]
pub struct SyncParams {
    force: Option<String>,
}

// 更新 sync_trigger 函数，显式指定返回类型
pub async fn sync_trigger(Query(params): Query<SyncParams>) -> Response {
    if let Some(force) = &params.force {
        if force == "true" {
            return force_sync().await;
        }
    }

    // 创建meetup对象
    // let meetup = Meetup::new(
    //     "1".to_string(),
    //     "Rust开发者聚会".to_string(),
    //     Some("讨论Rust和Axum框架的使用".to_string()),
    //     "2023-12-01".to_string(),
    //     "线上".to_string(),
    // );

    // println!("已创建Meetup: {:?}", meetup);

    // 返回成功信息
    (StatusCode::OK, "同步触发成功").into_response()
}

// 定义同步状态结构体
struct SyncState {
    is_running: bool,
    current_id: usize,
    success_count: usize,
}

// 全局共享状态
lazy_static::lazy_static! {
    static ref SYNC_STATE: Arc<Mutex<SyncState>> = Arc::new(Mutex::new(SyncState {
        is_running: false,
        current_id: 0,
        success_count: 0,
    }));
}

async fn force_sync() -> Response {
    // 尝试获取锁并检查是否已在运行
    let mut state = SYNC_STATE.lock().await;

    if state.is_running {
        // 如果已经在运行，返回当前状态
        let message = format!(
            "强制同步正在进行中，当前处理ID: {}，已成功处理: {} 个请求",
            state.current_id, state.success_count
        );
        println!("{}", message);
        return (StatusCode::OK, message).into_response();
    }

    // 标记为正在运行
    state.is_running = true;
    state.current_id = 0;
    state.success_count = 0;

    // 释放锁，这样其他请求可以查询状态
    drop(state);

    // 创建HTTP客户端
    let client = reqwest::Client::new();
    let base_url = "https://mahjong.chaotic.quest/sthlm-meetups-league-season0/table/";

    println!("开始强制同步过程，从ID 0开始请求...");

    let mut id = 0;
    let mut success_count = 0;

    loop {
        // 更新当前状态
        {
            let mut state = SYNC_STATE.lock().await;
            state.current_id = id;
            state.success_count = success_count;
        }

        let url = format!("{}{}/", base_url, id);
        println!("请求URL: {}", url);

        match client.get(&url).send().await {
            Ok(response) => {
                let status = response.status();
                if status == StatusCode::BAD_GATEWAY { // 502 Bad Gateway
                    println!("遇到502错误，停止请求。最后请求的ID为: {}", id);
                    break;
                } else if status.is_success() {
                    success_count += 1;
                    println!("ID {} 请求成功 ({})", id, status);
                    // 输出网页内容
                    let body = response.text().await.unwrap_or_else(|_| "无法获取响应内容".to_string());
                    println!("响应内容: {}", body);
                } else {
                    println!("ID {} 请求返回状态码: {}", id, status);
                }
            },
            Err(e) => {
                println!("请求ID {} 时发生错误: {}", id, e);
                // 对于网络错误，可能需要重试或暂停
                if e.is_timeout() || e.is_connect() {
                    println!("网络错误，暂停5秒后继续");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                break;
            }
        }

        id += 1;

        // 防止过快请求，添加小延迟
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // 同步完成，重置状态
    {
        let mut state = SYNC_STATE.lock().await;
        state.is_running = false;
    }

    println!("强制同步完成，成功请求数: {}", success_count);
    (StatusCode::OK, format!("强制同步触发成功，共处理{}个请求", success_count)).into_response()
}