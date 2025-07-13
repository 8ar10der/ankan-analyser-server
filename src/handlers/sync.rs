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
    let url = "https://mahjong.chaotic.quest/sthlm-meetups-league/data.json";

    println!("开始强制同步过程，拉取JSON数据...");

    // 拉取JSON数据
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("请求数据源失败: {}", e);
            println!("{}", msg);
            let mut state = SYNC_STATE.lock().await;
            state.is_running = false;
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
        }
    };
    let data: DataRoot = match resp.json().await {
        Ok(d) => d,
        Err(e) => {
            let msg = format!("解析JSON失败: {}", e);
            println!("{}", msg);
            let mut state = SYNC_STATE.lock().await;
            state.is_running = false;
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
        }
    };

    // 构建pid到玩家名映射
    let mut pid_name_map = HashMap::new();
    for p in &data.collection.players {
        pid_name_map.insert(p.pid, p.name.clone());
    }

    let mut success_count = 0;
    let mut saved_count = 0;

    // 获取现有玩家
    let existing_players = repo.list_players().await.unwrap_or_default();

    // 第一步：检查缺失的ID和不一致的姓名
    println!("开始检查玩家数据一致性...");

    // 构建现有玩家的ID到姓名映射
    let mut existing_id_name_map = HashMap::new();
    for player in &existing_players {
        existing_id_name_map.insert(player.id, player.name.clone());
    }

    let mut updated_players_count = 0;
    let mut created_players_count = 0;

    // 检查JSON中的每个玩家
    for json_player in &data.collection.players {
        let pid = json_player.pid as i32;
        let json_name = &json_player.name;

        if let Some(existing_name) = existing_id_name_map.get(&pid) {
            // ID存在，检查姓名是否一致
            if existing_name != json_name {
                println!("⚠️  发现ID {} 的姓名不一致：", pid);
                println!("   数据库中: '{}'", existing_name);
                println!("   JSON中:   '{}'", json_name);
                println!("   正在更新...");

                let update_player = LeaguePlayer::new(pid, json_name.clone());
                match repo.update_player(&update_player).await {
                    Ok(_) => {
                        println!("✅ 成功更新ID {} 的玩家姓名: '{}' -> '{}'", pid, existing_name, json_name);
                        // 更新本地映射
                        existing_id_name_map.insert(pid, json_name.clone());
                        updated_players_count += 1;
                    },
                    Err(e) => {
                        println!("❌ 更新玩家ID {} 姓名失败: {}", pid, e);
                    }
                }
            } else {
                println!("✓ ID {} 的玩家 '{}' 信息一致", pid, json_name);
            }
        } else {
            // ID不存在，需要创建新玩家
            println!("📝 发现缺失的玩家ID {}，姓名: {}，正在创建...", pid, json_name);
            let new_player = LeaguePlayer::new(pid, json_name.clone());
            match repo.create_player_with_id(&new_player).await {
                Ok(created_id) => {
                    println!("✅ 成功创建玩家: {} (ID: {})", json_name, created_id);
                    existing_id_name_map.insert(pid, json_name.clone());
                    created_players_count += 1;
                },
                Err(e) => {
                    println!("❌ 创建玩家ID {} ({}) 失败: {}", pid, json_name, e);
                }
            }
        }
    }

    println!("玩家数据一致性检查完成:");
    println!("  - 更新了 {} 个玩家的姓名", updated_players_count);
    println!("  - 创建了 {} 个新玩家", created_players_count);
    println!("  - 总共检查了 {} 个玩家", data.collection.players.len());

    // 构建姓名到ID的映射，用于后续的游戏处理
    let mut player_id_map = HashMap::new();
    for (id, name) in &existing_id_name_map {
        player_id_map.insert(name.clone(), *id);
    }

    for game in &data.collection.games {
        // 生成PlayerResult列表
        let mut player_results = Vec::new();
        for result in &game.results {
            let player_name = pid_name_map.get(&result.player).cloned().unwrap_or_else(|| "Unknown".to_string());
            let seat = result.seat.clone();
            let score = result.result;
            let position = result.position.unwrap_or(0) as i32;
            let uma = result.uma.unwrap_or(0.0);
            let penalty = result.penalty.unwrap_or(0.0);
            let total = result.total.unwrap_or(0.0);
            player_results.push(PlayerResult {
                seat,
                player_name,
                score,
                position,
                uma,
                penalty,
                total,
            });
        }

        // 优化桌号提取逻辑，兼容多种描述格式，失败时用gid兜底
        let mut season_num = 0;
        let mut table_num = 0;
        let desc = &game.description;
        // 支持多种格式：Season X: Table Y 或 Season X: Group ...: Table Y
        let re = Regex::new(r"Season (\d+)(?:: [^:]+)*: Table (\d+)").unwrap();
        if let Some(caps) = re.captures(desc) {
            season_num = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(0);
            table_num = caps.get(2).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(game.gid as i32);
        } else {
            // fallback: 尝试简单的 Season X 格式
            let simple_re = Regex::new(r"Season (\d+)").unwrap();
            if let Some(caps) = simple_re.captures(desc) {
                season_num = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(0);
            }
            table_num = game.gid as i32;
        }

        // 创建GameInfo对象
        let game_info = GameInfo {
            game_id: game.gid as i32,
            played_date: NaiveDate::parse_from_str(&game.played, "%Y-%m-%d").unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970,1,1).unwrap()),
            registered: None,
            description: game.description.clone(),
            processed: true,
            player_results,
            season_num,
            table_num,
        };

        // 步骤1：首先获取所有现有玩家，以便正确分配新ID
        for player_result in &game_info.player_results {
            let player_name = &player_result.player_name;
            if player_id_map.contains_key(player_name) {
                continue;
            }

            // 查找该玩家在JSON数据中的pid
            let player_pid = data.collection.players.iter()
                .find(|p| p.name == *player_name)
                .map(|p| p.pid as i32)
                .unwrap_or(-1);

            if player_pid != -1 {
                // 使用JSON中的pid作为数据库ID创建玩家
                let new_player = LeaguePlayer::new(player_pid, player_name.clone());
                match repo.create_player_with_id(&new_player).await {
                    Ok(created_id) => {
                        println!("创建新玩家: {} (ID: {}, 来自JSON pid: {})", player_name, created_id, player_pid);
                        player_id_map.insert(player_name.clone(), player_pid);
                    },
                    Err(e) => {
                        // 检查是否是ID冲突错误
                        if e.to_string().contains("duplicate key") || e.to_string().contains("unique constraint") {
                            println!("ID {} 已存在，更新该ID对应的玩家名字为: {}", player_pid, player_name);
                            // ID冲突时，更新已存在ID的玩家名字
                            let update_player = LeaguePlayer::new(player_pid, player_name.clone());
                            match repo.update_player(&update_player).await {
                                Ok(_) => {
                                    println!("成功更新ID {} 的玩家名字为: {}", player_pid, player_name);
                                    player_id_map.insert(player_name.clone(), player_pid);
                                },
                                Err(update_e) => {
                                    println!("更新玩家 {} (ID: {}) 失败: {}，尝试自动分配ID", player_name, player_pid, update_e);
                                    // 如果更新也失败，则使用自动分配ID的方式
                                    let fallback_player = LeaguePlayer::new(-1, player_name.clone());
                                    if let Ok(new_player_id) = repo.create_player(&fallback_player).await {
                                        println!("创建新玩家 {} 成功 (自动分配ID: {})", player_name, new_player_id);
                                        player_id_map.insert(player_name.clone(), new_player_id);
                                    }
                                }
                            }
                        } else {
                            println!("使用指定ID创建玩家 {} 失败: {}，尝试自动分配ID", player_name, e);
                            // 其他错误，使用自动分配ID的方式
                            let fallback_player = LeaguePlayer::new(-1, player_name.clone());
                            if let Ok(new_player_id) = repo.create_player(&fallback_player).await {
                                println!("创建新玩家 {} 成功 (自动分配ID: {})", player_name, new_player_id);
                                player_id_map.insert(player_name.clone(), new_player_id);
                            }
                        }
                    }
                }
            } else {
                println!("警告: 玩家 {} 在JSON数据中找不到对应的pid，使用自动分配ID", player_name);
                let fallback_player = LeaguePlayer::new(-1, player_name.clone());
                if let Ok(new_player_id) = repo.create_player(&fallback_player).await {
                    println!("创建新玩家 {} 成功 (自动分配ID: {})", player_name, new_player_id);
                    player_id_map.insert(player_name.clone(), new_player_id);
                }
            }
        }

        // 步骤2：创建/更新游戏记录，使用已获取的玩家ID
        let mut e_id = 0;
        let mut s_id = 0;
        let mut w_id = 0;
        let mut n_id = 0;
        for player_result in &game_info.player_results {
            if let Some(&player_id) = player_id_map.get(&player_result.player_name) {
                // 统一seat匹配，去除括号并大写
                let seat = player_result.seat.trim_matches(|c| c == '[' || c == ']').to_uppercase();
                match seat.as_str() {
                    "E" | "EAST" => e_id = player_id,
                    "S" | "SOUTH" => s_id = player_id,
                    "W" | "WEST" => w_id = player_id,
                    "N" | "NORTH" => n_id = player_id,
                    _ => {}
                }
            }
        }

        let mut game_db = LeagueGame::new(
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

        let existing_game = repo.get_game_by_season_and_table(game_info.season_num, game_info.table_num).await;
        match existing_game {
            Ok(existing_game) => {
                println!("发现相同赛季({})和桌号({})的游戏记录，ID: {}，将进行更新",
                         game_info.season_num, game_info.table_num, existing_game.id);
                game_db = LeagueGame::new(
                    game_info.registered,
                    game_info.season_num,
                    game_info.table_num,
                    game_info.processed,
                    existing_game.id,
                    e_id,
                    s_id,
                    w_id,
                    n_id,
                );
                let _ = repo.update_game(&game_db).await;
            },
            Err(_) => {
                if let Ok(new_game_id) = repo.create_game(&game_db).await {
                    println!("游戏保存成功: ID {}", new_game_id);
                    saved_count += 1;
                    game_db.id = new_game_id;
                }
            }
        }

        // 步骤3：创建/更新玩家成绩
        if game_db.id >= 0 {
            for result in &game_info.player_results {
                let player_id = {
                    let seat = result.seat.trim_matches(|c| c == '[' || c == ']').to_uppercase();
                    match seat.as_str() {
                        "E" | "EAST" => game_db.e,
                        "S" | "SOUTH" => game_db.s,
                        "W" | "WEST" => game_db.w,
                        "N" | "NORTH" => game_db.n,
                        _ => continue,
                    }
                };

                let game_result = LeagueResult::new(
                    0,
                    game_db.id,
                    player_id,
                    result.score,
                    result.position,
                    result.uma,
                    result.penalty,
                    result.total
                );

                match repo.get_result_by_table_and_player(game_db.id, player_id).await {
                    Ok(mut existing_result) => {
                        existing_result.result = result.score;
                        existing_result.position = result.position;
                        existing_result.uma = result.uma;
                        existing_result.penalty = result.penalty;
                        existing_result.total = result.total;
                        let _ = repo.update_result(&existing_result).await;
                    },
                    Err(_) => {
                        let _ = repo.create_result(&game_result).await;
                    }
                }
            }
        }

        success_count += 1;
        // 更新状态
        let mut state = SYNC_STATE.lock().await;
        state.current_id = game.gid;
        state.success_count = success_count;
    }

    // 同步完成，重置状态
    {
        let mut state = SYNC_STATE.lock().await;
        state.is_running = false;
    }
    println!("强制同步完成，成功处理数: {}，成功保存数: {}", success_count, saved_count);
    (StatusCode::OK, format!("强制同步触发成功，共处理{}场比赛，成功保存{}条记录", success_count, saved_count)).into_response()
}

// 适配 data.json 的结构体 - 仅用于JSON反序列化
#[derive(Debug, Deserialize)]
struct DataRoot {
    collection: DataCollection,
}

#[derive(Debug, Deserialize)]
struct DataCollection {
    players: Vec<DataPlayer>,
    games: Vec<DataGame>,
    #[allow(dead_code)]
    sessions: Vec<DataSession>,
}

#[derive(Debug, Deserialize)]
struct DataPlayer {
    pid: usize,
    name: String,
}

#[derive(Debug, Deserialize)]
struct DataGame {
    gid: usize,
    played: String,
    description: String,
    #[allow(dead_code)]
    players: Vec<usize>,
    results: Vec<DataResult>,
}

#[derive(Debug, Deserialize)]
struct DataResult {
    player: usize,
    result: f64,
    seat: String,
    uma: Option<f64>,
    position: Option<u8>,
    penalty: Option<f64>,
    total: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DataSession {
    #[allow(dead_code)]
    sid: usize,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    group: String,
    #[allow(dead_code)]
    date: String,
    #[allow(dead_code)]
    games: Vec<usize>,
}

// dry run: 只返回将要同步的比赛和玩家信息，不写数据库
pub async fn dry_run_sync(State(repo): State<LeagueRepository>) -> Response {
    let client = reqwest::Client::new();
    let url = "https://mahjong.chaotic.quest/sthlm-meetups-league/data.json";
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("请求数据源失败: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
        }
    };
    let data: DataRoot = match resp.json().await {
        Ok(d) => d,
        Err(e) => {
            let msg = format!("解析JSON失败: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
        }
    };

    // 构建pid到玩家名映射
    let mut pid_name_map = HashMap::new();
    for p in &data.collection.players {
        pid_name_map.insert(p.pid, p.name.clone());
    }

    // 获取现有玩家
    let existing_players = repo.list_players().await.unwrap_or_default();
    let mut existing_player_names = std::collections::HashSet::new();
    for player in &existing_players {
        existing_player_names.insert(player.name.clone());
    }

    // 统计信息
    let mut all_players = std::collections::BTreeSet::new();
    let mut new_players = std::collections::BTreeSet::new();
    let mut games_info = Vec::new();
    let mut warnings = Vec::new();
    let mut season_stats = std::collections::HashMap::new();

    for game in &data.collection.games {
        // 解析赛季和桌号
        let mut season_num = 0;
        let mut table_num = 0;
        let desc = &game.description;
        let re = Regex::new(r"Season (\d+)(?:: [^:]+)*: Table (\d+)").unwrap();
        if let Some(caps) = re.captures(desc) {
            season_num = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(0);
            table_num = caps.get(2).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(game.gid as i32);
        } else {
            let simple_re = Regex::new(r"Season (\d+)").unwrap();
            if let Some(caps) = simple_re.captures(desc) {
                season_num = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(0);
            }
            table_num = game.gid as i32;
        }

        // 统计赛季信息
        *season_stats.entry(season_num).or_insert(0) += 1;

        // 分析玩家和座位
        let mut game_players = Vec::new();
        let mut seat_assignment = std::collections::HashMap::new();

        for result in &game.results {
            let player_name = pid_name_map.get(&result.player).cloned().unwrap_or_else(|| {
                warnings.push(format!("游戏 {} 中找不到玩家 ID {}", game.gid, result.player));
                format!("Unknown_{}", result.player)
            });

            all_players.insert(player_name.clone());
            if !existing_player_names.contains(&player_name) {
                new_players.insert(player_name.clone());
            }

            // 处理座位
            let seat = result.seat.trim_matches(|c| c == '[' || c == ']').to_uppercase();
            let normalized_seat = match seat.as_str() {
                "E" | "EAST" => "东",
                "S" | "SOUTH" => "南",
                "W" | "WEST" => "西",
                "N" | "NORTH" => "北",
                _ => {
                    warnings.push(format!("游戏 {} 中玩家 {} 的座位无法识别: {}", game.gid, player_name, result.seat));
                    "未知"
                }
            };

            if seat_assignment.contains_key(normalized_seat) {
                warnings.push(format!("游戏 {} 中座位 {} 被分配给多个玩家", game.gid, normalized_seat));
            }
            seat_assignment.insert(normalized_seat, player_name.clone());

            game_players.push(serde_json::json!({
                "name": player_name,
                "seat": normalized_seat,
                "original_seat": result.seat,
                "score": result.result,
                "position": result.position.unwrap_or(0),
                "uma": result.uma.unwrap_or(0.0),
                "penalty": result.penalty.unwrap_or(0.0),
                "total": result.total.unwrap_or(0.0),
                "is_new_player": !existing_player_names.contains(&player_name)
            }));
        }

        // 检查是否有4个玩家
        if game_players.len() != 4 {
            warnings.push(format!("游戏 {} 玩家数量不是4个: {}", game.gid, game_players.len()));
        }

        // 检查是否所有座位都被占用
        let expected_seats = ["东", "南", "西", "北"];
        for seat in &expected_seats {
            if !seat_assignment.contains_key(*seat) {
                warnings.push(format!("游戏 {} 缺少 {} 座位的玩家", game.gid, seat));
            }
        }

        games_info.push(serde_json::json!({
            "gid": game.gid,
            "played": game.played,
            "description": game.description,
            "season_num": season_num,
            "table_num": table_num,
            "players": game_players,
            "seat_assignment": seat_assignment
        }));
    }

    let result = serde_json::json!({
        "summary": {
            "total_games": data.collection.games.len(),
            "total_players": all_players.len(),
            "new_players_count": new_players.len(),
            "existing_players_count": all_players.len() - new_players.len(),
            "warnings_count": warnings.len(),
            "season_distribution": season_stats
        },
        "players": {
            "all_players": all_players,
            "new_players": new_players,
            "existing_players": all_players.difference(&new_players).collect::<std::collections::BTreeSet<_>>()
        },
        "warnings": warnings,
        "games_preview": games_info.iter().take(10).collect::<Vec<_>>(),
        "games_by_season": {
            "season_0": games_info.iter().filter(|g| g["season_num"] == 0).count(),
            "season_1": games_info.iter().filter(|g| g["season_num"] == 1).count(),
            "other_seasons": games_info.iter().filter(|g| g["season_num"] != 0 && g["season_num"] != 1).count()
        }
    });

    (StatusCode::OK, axum::Json(result)).into_response()
}
