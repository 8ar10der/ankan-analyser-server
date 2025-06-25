use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use crate::db::LeagueRepository;
use crate::models::league::{GameInfo, PlayerResult, LeaguePlayer, LeagueGame, LeagueResult};
use std::sync::Arc;
use tokio::sync::Mutex;
use scraper::{Html, Selector};
use chrono::NaiveDate;
use regex::Regex;
use std::collections::HashMap;

pub async fn hello() -> impl IntoResponse {
    (StatusCode::OK, "Hello, World!")
}

// 定义查询参数结构体
#[derive(Deserialize)]
pub struct SyncParams {
    force: Option<String>,
}

// 更新 sync_trigger 函数，接收LeagueRepository作为状态
pub async fn sync_trigger(
    State(repo): State<LeagueRepository>,
    Query(params): Query<SyncParams>
) -> Response {
    if let Some(force) = &params.force {
        if force == "true" {
            return force_sync(repo).await;
        }
    }

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

// 修改force_sync函数以接收LeagueRepository
async fn force_sync(repo: LeagueRepository) -> Response {
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
    let mut saved_count = 0;

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

                    // 获取HTML内容
                    let body = response.text().await.unwrap_or_else(|_| "无法获取响应内容".to_string());

                    // 解析HTML内容并创建GameInfo对象
                    if let Some(game_info) = parse_html_content(&body) {
                        println!("成功解析游戏信息: Game ID {}, 日期 {}, 赛季 {}, 桌号 {}",
                                 game_info.game_id, game_info.played_date, game_info.season_num, game_info.table_num);

                        // 步骤1：首先获取所有现有玩家，以便正确分配新ID
                        let mut player_id_map = HashMap::new(); // 存储玩家名称到真实数据库ID的映射

                        // 首先尝试获取数据库中当前的最大玩家ID
                        let existing_players = repo.list_players().await.unwrap_or_else(|e| {
                            println!("获取现有玩家列表时出错: {}", e);
                            vec![]
                        });

                        // 创建现有玩家名称到ID的映射
                        for player in &existing_players {
                            player_id_map.insert(player.name.clone(), player.id);
                        }

                        // 处理游戏中的每个玩家
                        for player_result in &game_info.player_results {
                            let player_name = &player_result.player_name;

                            // 检查玩家是否已存在于映射中（数据库中已有记录）
                            if player_id_map.contains_key(player_name) {
                                println!("玩家已存在: {} (ID: {})", player_name, player_id_map[player_name]);
                                continue;
                            }

                            // 玩家不存在，创建新玩家，分配新ID

                            let new_player = LeaguePlayer::new(-1, player_name.clone());
                            match repo.create_player(&new_player).await {
                                Ok(new_player_id) => {
                                    println!("创建新玩家: {} (ID: {})", player_name, new_player_id);
                                    player_id_map.insert(player_name.clone(), new_player_id);
                                },
                                Err(e) => {
                                    println!("创建玩家时出错: {}", e);
                                }
                            }
                        }

                        // 步骤2：创建/更新游戏记录，使用已获取的玩家ID
                        // 分配座位ID (e, s, w, n)
                        let mut e_id = 0;
                        let mut s_id = 0;
                        let mut w_id = 0;
                        let mut n_id = 0;

                        for player_result in &game_info.player_results {
                            if let Some(&player_id) = player_id_map.get(&player_result.player_name) {
                                match player_result.seat.as_str() {
                                    "[E]" => e_id = player_id,
                                    "[S]" => s_id = player_id,
                                    "[W]" => w_id = player_id,
                                    "[N]" => n_id = player_id,
                                    _ => {}
                                }
                            }
                        }

                        // 创建对局信息对象
                        let mut game = LeagueGame::new(
                            game_info.registered,
                            game_info.season_num,
                            game_info.table_num,
                            game_info.processed,
                            game_info.game_id,
                            e_id,
                            s_id,
                            w_id,
                            n_id,
                        );

                        // 首先检查是否已存在相同赛季和桌号的记录
                        let existing_game = repo.get_game_by_season_and_table(game_info.season_num, game_info.table_num).await;

                        match existing_game {
                            Ok(existing_game) => {
                                // 已存在相同赛季和桌号的游戏，进行更新
                                println!("发现相同赛季({})和桌号({})的游戏记录，ID: {}，将进行更新",
                                         game_info.season_num, game_info.table_num, existing_game.id);

                                // 使用现有的游戏ID，但更新其他信息
                                game = LeagueGame::new(
                                    game_info.registered,
                                    game_info.season_num,
                                    game_info.table_num,
                                    game_info.processed,
                                    existing_game.id, // 保持原有ID不变
                                    e_id,
                                    s_id,
                                    w_id,
                                    n_id,
                                );

                                // 更新游戏信息
                                match repo.update_game(&game).await {
                                    Ok(_) => {
                                        println!("游戏信息已更新: ID {}", game.id);
                                    },
                                    Err(e) => {
                                        println!("更新游戏时出错: {}", e);
                                    }
                                }
                            },
                            Err(_) => {
                                // 不存在相同赛季和桌号的游戏，进行创建

                                // 保存游戏到数据库并获取ID更新到game对象
                                match repo.create_game(&game).await {
                                    Ok(new_game_id) => {
                                        println!("游戏保存成功: ID {}", new_game_id);
                                        saved_count += 1; // 成功保存游戏记录
                                        // 更新游戏ID
                                        game.id = new_game_id;
                                    },
                                    Err(e) => {
                                        if e.to_string().contains("duplicate key") {
                                            println!("游戏已存在: ID {}", game.id);
                                        } else {
                                            println!("创建游戏时出错: {}", e);
                                        }
                                    }
                                }
                            }
                        }

                        // 步骤3：创建更新玩家成绩记录
                        if game.id >= 0 {
                            println!("开始处理游戏 ID {}，赛季 {}，桌号 {} 的玩家成绩",
                                    game.id, game.season_num, game.table_num);

                            // 处理四个座位的玩家成绩
                            for result in &game_info.player_results {
                                // 根据座位位置找到对应的玩家ID
                                let player_id = match result.seat.as_str() {
                                    "[E]" => game.e,
                                    "[S]" => game.s,
                                    "[W]" => game.w,
                                    "[N]" => game.n,
                                    _ => {
                                        println!("无效的座位位置: {}", result.seat);
                                        continue;
                                    }
                                };

                                // 创建游戏成绩对象
                                let game_result = LeagueResult::new(
                                    0, // ID设为0，数据库会自动分配
                                    game.id,
                                    player_id,
                                    result.score,
                                    result.position,
                                    result.uma,
                                    result.penalty,
                                    result.total
                                );

                                // 先检查是否已存在该玩家在该桌的成绩记录
                                match repo.get_result_by_table_and_player(game.id, player_id).await {
                                    Ok(existing_result) => {
                                        // 已有记录，进行更新
                                        let mut updated_result = existing_result;
                                        updated_result.result = result.score;
                                        updated_result.position = result.position;
                                        updated_result.uma = result.uma;
                                        updated_result.penalty = result.penalty;
                                        updated_result.total = result.total;

                                        match repo.update_result(&updated_result).await {
                                            Ok(_) => {
                                                println!("已更新玩家 {} 在桌号 {} 的成绩", player_id, game.id);
                                            },
                                            Err(e) => {
                                                println!("更新玩家 {} 成绩时出错: {}", player_id, e);
                                            }
                                        }
                                    },
                                    Err(_) => {
                                        // 没有找到记录，创建新记录
                                        match repo.create_result(&game_result).await {
                                            Ok(_) => {
                                                println!("已保存玩家 {} 在桌号 {} 的成绩", player_id, game.id);
                                            },
                                            Err(e) => {
                                                println!("保存玩家 {} 成绩时出错: {}", player_id, e);
                                            }
                                        }
                                    }
                                }
                            }

                            // 标记游戏为已处理状态
                            if !game.processed {
                                let mut processed_game = game.clone();
                                processed_game.processed = true;

                                match repo.update_game(&processed_game).await {
                                    Ok(_) => {
                                        println!("游戏 ID {} 已标记为处理完成", game.id);
                                    },
                                    Err(e) => {
                                        println!("标记游戏 ID {} 为已处理状态时出错: {}", game.id, e);
                                    }
                                }
                            }
                        }
                    } else {
                        println!("解析游戏信息失败，ID: {}", id);
                    }
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
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }

    // 同步完成，重置状态
    {
        let mut state = SYNC_STATE.lock().await;
        state.is_running = false;
    }
    println!("强制同步完成，成功请求数: {}，成功保存数: {}", success_count, saved_count);
    (StatusCode::OK, format!("强制同步触发成功，共处理{}个请求，成功保存{}条记录", success_count, saved_count)).into_response()
}

// 添加HTML解析功能
fn parse_html_content(html: &str) -> Option<GameInfo> {
    let document = Html::parse_document(html);

    // 获取游戏基本信息
    let game_id_selector = Selector::parse("tr.row:nth-child(2) td.cell.value").ok()?;
    let game_id_element = document.select(&game_id_selector).next()?;
    let game_id_html = game_id_element.inner_html();
    let game_id_text = game_id_html.trim();
    let game_id = game_id_text.parse::<i32>().ok()?;

    // 获取比赛日期
    let played_date_selector = Selector::parse("tr.row:nth-child(3) td[clalss='cell value']").ok()?;
    let played_date_element = document.select(&played_date_selector).next()?;
    let played_date_html = played_date_element.inner_html();
    let played_date_text = played_date_html.trim();
    let played_date = NaiveDate::parse_from_str(played_date_text, "%Y-%m-%d").ok()?;

    // 获取注册时间
    let registered_selector = Selector::parse("tr.row:nth-child(4) td[clalss='cell value']").ok()?;
    let registered_element = document.select(&registered_selector).next()?;
    let registered_html = registered_element.inner_html();
    let registered_text = registered_html.trim();
    // 注册时间为可选项，解析失败不应该导致整个函数失败
    let registered = chrono::NaiveDateTime::parse_from_str(registered_text, "%Y-%m-%d %H:%M:%S%.f %Z").ok();

    // 获取描述
    let description_selector = Selector::parse("tr.row:nth-child(5) td[clalss='cell value']").ok()?;
    let description_element = document.select(&description_selector).next()?;
    let description_html = description_element.inner_html();
    let description_text = description_html.trim().to_string();

    // 从描述中提取赛季和桌号
    let re = Regex::new(r"Season (\d+): Table (\d+)").ok()?;
    let caps = re.captures(&description_text)?;
    let season_num = caps.get(1)?.as_str().parse::<i32>().ok()?;
    let table_num = caps.get(2)?.as_str().parse::<i32>().ok()?;

    // 获取处理状态
    let processed_selector = Selector::parse("tr.row:nth-child(6) td[clalss='cell value']").ok()?;
    let processed_element = document.select(&processed_selector).next()?;
    let processed_html = processed_element.inner_html();
    let processed_text = processed_html.trim();
    let processed = processed_text == "true";

    // 获取玩家成绩
    let mut player_results = Vec::new();

    // 选择所有玩家行
    let player_row_selector = Selector::parse("table:nth-child(2) tr.row:not(.header)").ok()?;
    for row in document.select(&player_row_selector) {
        // 获取座位
        let seat_selector = Selector::parse("td.cell.seat").ok()?;
        let seat_element = row.select(&seat_selector).next()?;
        let seat_html = seat_element.inner_html();
        let seat = seat_html.trim().to_string();

        // 获取玩家名称
        let name_selector = Selector::parse("td.cell.name").ok()?;
        let name_element = row.select(&name_selector).next()?;
        let name_html = name_element.inner_html();
        let player_name = name_html.trim().to_string();

        // 获取分数
        let score_selector = Selector::parse("td.cell.score").ok()?;
        let score_element = row.select(&score_selector).next()?;
        let score_html = score_element.inner_html();
        let score_text = score_html.trim().replace(" ", "");
        let score = score_text.parse::<f64>().ok()?;

        // 获取位置
        let position_selector = Selector::parse("td.cell.position").ok()?;
        let position_element = row.select(&position_selector).next()?;
        let position_html = position_element.inner_html();
        let position_text = position_html.trim();
        let position = position_text.parse::<i32>().ok()?;

        // 获取马点
        let uma_selector = Selector::parse("td.cell.uma").ok()?;
        let uma_element = row.select(&uma_selector).next()?;
        let uma_html = uma_element.inner_html();
        let uma_text = uma_html.trim().replace(" ", "");
        let uma = uma_text.parse::<f64>().ok()?;

        // 获取罚分
        let penalty_selector = Selector::parse("td.cell.penalty").ok()?;
        let penalty_element = row.select(&penalty_selector).next()?;
        let penalty_html = penalty_element.inner_html();
        let penalty_text = penalty_html.trim().replace(" ", "");
        let penalty = penalty_text.parse::<f64>().unwrap_or(0.0);

        // 获取总分
        let total_selector = Selector::parse("td.cell.total").ok()?;
        let total_element = row.select(&total_selector).next()?;
        let total_html = total_element.inner_html();
        let total_text = total_html.trim().replace(" ", "");
        let total = total_text.parse::<f64>().ok()?;

        // 创建PlayerResult对象
        let player_result = PlayerResult {
            seat,
            player_name,
            score,
            position,
            uma,
            penalty,
            total,
        };

        player_results.push(player_result);
    }

    // 创建GameInfo对象
    Some(GameInfo {
        game_id,
        played_date,
        registered,
        description: description_text,
        processed,
        player_results,
        season_num,
        table_num,
    })
}