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
                        let existing_players = match repo.list_players().await {
                            Ok(players) => players,
                            Err(e) => {
                                println!("获取现有玩家列表时出错: {}", e);
                                vec![]
                            }
                        };

                        // 创建现有玩家名称到ID的映射
                        let mut max_player_id = 0;
                        for player in &existing_players {
                            player_id_map.insert(player.name.clone(), player.id);
                            max_player_id = std::cmp::max(max_player_id, player.id);
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
                            max_player_id += 1;
                            let new_player_id = max_player_id;
                            let new_player = LeaguePlayer::new(new_player_id, player_name.clone());

                            match repo.create_player(&new_player).await {
                                Ok(_) => {
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
                        let mut n_id = None;

                        for player_result in &game_info.player_results {
                            if let Some(&player_id) = player_id_map.get(&player_result.player_name) {
                                match player_result.seat.as_str() {
                                    "[E]" => e_id = player_id,
                                    "[S]" => s_id = player_id,
                                    "[W]" => w_id = player_id,
                                    "[N]" => n_id = Some(player_id),
                                    _ => {}
                                }
                            }
                        }

                        // 首先检查是否已存在相同赛季和桌号的记录
                        let existing_game = repo.get_game_by_season_and_table(game_info.season_num, game_info.table_num).await;

                        let mut game_exists = false;
                        let mut existing_game_id = 0;

                        match existing_game {
                            Ok(game) => {
                                // 已存在相同赛季和桌号的游戏，进行更新
                                game_exists = true;
                                existing_game_id = game.id;
                                println!("发现相同赛季({})和桌号({})的游戏记录，ID: {}，将进行更新",
                                         game_info.season_num, game_info.table_num, game.id);

                                // 使用现有的游戏ID，但更新其他信息
                                let updated_game = LeagueGame::new(
                                    game_info.registered,
                                    game_info.season_num,
                                    game_info.table_num,
                                    game_info.processed,
                                    game.id, // 保持原有ID不变
                                    e_id,
                                    s_id,
                                    w_id,
                                    n_id,
                                );

                                // 更新游戏信息
                                match repo.update_game(&updated_game).await {
                                    Ok(_) => {
                                        println!("游戏信息已更新: ID {}", updated_game.id);

                                        // 先删除所有与此游戏相关的旧结果记录
                                        match repo.delete_results_by_table_id(updated_game.id).await {
                                            Ok(result) => {
                                                println!("已删除 {} 条旧结果记录", result.rows_affected());
                                            },
                                            Err(e) => {
                                                println!("删除旧结果记录时出错: {}", e);
                                            }
                                        }

                                        // 步骤3：保存游戏结果，使用已获取的玩家ID和游戏ID
                                        let mut result_saved = 0;
                                        let max_result_id = match repo.get_max_result_id().await {
                                            Ok(id) => id.unwrap_or(0),
                                            Err(e) => {
                                                println!("获取结果表最大ID时出错: {}", e);
                                                0
                                            }
                                        };

                                        println!("当前结果表最大ID: {}", max_result_id);
                                        let mut result_counter = max_result_id + 1;

                                        for player_result in &game_info.player_results {
                                            if let Some(&player_id) = player_id_map.get(&player_result.player_name) {
                                                // 创建结果对象，使用基于当前数据库最大ID的计数器
                                                let result = LeagueResult::new(
                                                    result_counter,
                                                    updated_game.id, // 使用更新的游戏ID
                                                    player_id,
                                                    player_result.score,
                                                    player_result.position,
                                                    player_result.uma,
                                                    player_result.penalty,
                                                    player_result.total,
                                                );

                                                // 保存结果
                                                match repo.create_result(&result).await {
                                                    Ok(_) => {
                                                        result_saved += 1;
                                                        result_counter += 1;
                                                        println!("成功保存结果: ID {} 玩家 {} (ID: {})", result.id, player_result.player_name, player_id);
                                                    },
                                                    Err(e) => {
                                                        println!("保存结果时出错: 玩家 {} (ID: {}), 错误: {}", player_result.player_name, player_id, e);
                                                    }
                                                };
                                            }
                                        }

                                        println!("为游戏ID {} 更新并保存了 {}/{} 条结果记录",
                                                 updated_game.id, result_saved, game_info.player_results.len());

                                        if result_saved > 0 {
                                            saved_count += 1;
                                        }
                                    },
                                    Err(e) => {
                                        println!("更新游戏时出错: {}", e);
                                    }
                                }
                            },
                            Err(_) => {
                                // 不存在相同赛季和桌号的游戏，进行创建
                                // 创建游戏对象
                                let game = LeagueGame::new(
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

                                // 保存游戏到数据库
                                match repo.create_game(&game).await {
                                    Ok(_) => {
                                        println!("游戏保存成功: ID {}", game.id);

                                        // 步骤3：保存游戏结果，使用已获取的玩家ID和游戏ID

                                        // 获取当前结果表中的最大ID
                                        let max_result_id = match repo.get_max_result_id().await {
                                            Ok(id) => id.unwrap_or(0),
                                            Err(e) => {
                                                println!("获取结果表最大ID时出错: {}", e);
                                                0
                                            }
                                        };

                                        println!("当前结果表最大ID: {}", max_result_id);
                                        let mut result_saved = 0;
                                        let mut result_counter = max_result_id + 1;

                                        for player_result in &game_info.player_results {
                                            if let Some(&player_id) = player_id_map.get(&player_result.player_name) {
                                                // 创建结果对象，使用基于当前数据库最大ID的计数器
                                                let result = LeagueResult::new(
                                                    result_counter,
                                                    game.id,
                                                    player_id,
                                                    player_result.score,
                                                    player_result.position,
                                                    player_result.uma,
                                                    player_result.penalty,
                                                    player_result.total,
                                                );

                                                // 保存结果
                                                match repo.create_result(&result).await {
                                                    Ok(_) => {
                                                        result_saved += 1;
                                                        result_counter += 1;
                                                        println!("成功保存结果: ID {} 玩家 {} (ID: {})", result.id, player_result.player_name, player_id);
                                                    },
                                                    Err(e) => {
                                                        println!("保存结果时出错: 玩家 {} (ID: {}), 错误: {}", player_result.player_name, player_id, e);
                                                    }
                                                };
                                            }
                                        }

                                        println!("为游戏ID {} 保存了 {}/{} 条结果记录",
                                                 game.id, result_saved, game_info.player_results.len());

                                        if result_saved > 0 {
                                            saved_count += 1;
                                        }
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
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
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

// 添加函数将GameInfo转换为数据库实体对象
fn convert_game_info_to_entities(game_info: &GameInfo) -> (LeagueGame, HashMap<String, i32>, Vec<LeagueResult>) {
    // 创建玩家映射，用于存储玩家名称到ID的映射
    let mut player_map = HashMap::new();
    let mut result_counter = 1; // 用于生成结果ID

    // 为每个玩家生成一个唯一ID（简单实现，实际应用中可能需要更复杂的逻辑）
    for (i, player) in game_info.player_results.iter().enumerate() {
        player_map.insert(player.player_name.clone(), (i + 1) as i32);
    }

    // 创建LeagueGame对象
    let mut e_id = 0;
    let mut s_id = 0;
    let mut w_id = 0;
    let mut n_id = None;

    // 遍历玩家分配座位ID
    for player in &game_info.player_results {
        let player_id = player_map.get(&player.player_name).unwrap_or(&0);
        match player.seat.as_str() {
            "[E]" => e_id = *player_id,
            "[S]" => s_id = *player_id,
            "[W]" => w_id = *player_id,
            "[N]" => n_id = Some(*player_id),
            _ => {}
        }
    }

    // 创建游戏对象
    let game = LeagueGame::new(
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

    // 创建结果对象列表
    let mut results = Vec::new();
    for player in &game_info.player_results {
        if let Some(&player_id) = player_map.get(&player.player_name) {
            let result = LeagueResult::new(
                result_counter,
                game_info.game_id,
                player_id,
                player.score,
                player.position,
                player.uma,
                player.penalty,
                player.total,
            );
            results.push(result);
            result_counter += 1;
        }
    }

    (game, player_map, results)
}
