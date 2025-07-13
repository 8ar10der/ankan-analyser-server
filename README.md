# Ankan Meetup Analyser Server / Ankan俱乐部比赛分析服务器

[English](#english) | [中文](#中文)

---

## English

A Rust server application for analyzing ankan mahjong games data. This project synchronizes game data from remote JSON sources and provides RESTful API interfaces for querying player statistics and match records.

### 🎯 Features

- **Data Synchronization**: Automatically sync game data from `https://mahjong.chaotic.quest/sthlm-meetups-league/data.json`
- **Player Management**: Automatically maintain player information with ID consistency checks and name updates
- **Match Records**: Store and manage multi-season game data
- **Statistical Analysis**: Provide player performance statistics and ranking features
- **RESTful API**: Complete HTTP API interface
- **Data Integrity**: Intelligent handling of ID conflicts and data consistency issues

### 🛠 Tech Stack

- **Rust** - Primary programming language
- **Axum** - Async web framework
- **SQLx** - Async SQL toolkit
- **PostgreSQL** - Database
- **Serde** - JSON serialization/deserialization
- **Tokio** - Async runtime
- **Reqwest** - HTTP client

### 📊 Data Structure

#### Core Entities

- **LeaguePlayer**: Player information (ID, name)
- **LeagueGame**: Game information (season, table number, player seats)
- **LeagueResult**: Game results (score, rank, uma, penalty)

#### Data Source Format

The project supports standard JSON format from Stockholm Meetup League:

```json
{
  "collection": {
    "players": [
      {"pid": 0, "name": "Player Name", ...}
    ],
    "games": [
      {
        "gid": 0,
        "played": "2025-01-01",
        "description": "Season 0: Table 1",
        "results": [...]
      }
    ]
  }
}
```

### 🚀 Quick Start

#### Requirements

- Rust 1.70+
- PostgreSQL 12+
- Network connection (for data synchronization)

#### Installation

1. **Clone the project**
```bash
git clone <repository-url>
cd ankan-meetup-analyser-server
```

2. **Configure database**
```bash
# Create database
createdb ankan_meetup_league

# Set database URL environment variable
export DATABASE_URL="postgresql://username:password@localhost/ankan_meetup_league"
```

3. **Run database migrations**
```bash
sqlx migrate run
```

4. **Build and run**
```bash
cargo run
```

The server will start at `http://localhost:3000`.

### 📡 API Endpoints

#### Basic Endpoints

- `GET /` - Service health check
- `GET /hello` - Hello World test

#### Data Synchronization

- `GET /sync?force=true` - Force sync all data
- `GET /sync/dry-run` - Preview sync operations (without execution)

#### Player API

- `GET /api/players` - Get all players list
- `GET /api/players/{name}` - Get specific player information
- `GET /api/players/{name}/matches` - Get all player match records
- `GET /api/players/{name}/matches/{season}` - Get player records for specific season

#### Season API

- `GET /api/seasons` - Get all seasons list
- `GET /api/seasons/{season}/players` - Get players list for specific season

### 🔄 Data Synchronization Mechanism

#### Sync Process

1. **Player Data Consistency Check**
   - Check if player IDs from JSON exist in database
   - Compare player names for same IDs
   - Auto-update outdated player names
   - Create missing player records

2. **Game Data Processing**
   - Parse season and table number information
   - Handle player seat assignments (East, South, West, North)
   - Calculate and store game results

3. **Data Integrity Assurance**
   - Use transactions to ensure data consistency
   - Intelligent handling of ID conflicts
   - Support incremental updates

#### Sync Examples

```bash
# Execute full sync
curl "http://localhost:3000/sync?force=true"

# Preview sync operations
curl "http://localhost:3000/sync/dry-run"
```

### 📁 Project Structure

```
src/
├── main.rs              # Application entry point
├── routes.rs            # Route configuration
├── db/
│   ├── mod.rs          # Database module
│   └── ankan.rs        # Database operations implementation
├── handlers/
│   ├── mod.rs          # Handler module
│   ├── sync.rs         # Data sync handler
│   └── league_api.rs   # API handler
└── models/
    ├── mod.rs          # Model module
    └── league.rs       # Data model definitions
```

### 🔧 Configuration

#### Environment Variables

- `DATABASE_URL` - PostgreSQL connection string
- `PORT` - Server port (default 3000)
- `RUST_LOG` - Log level

#### Data Source Configuration

Current data source is hardcoded for Stockholm Mahjong League:
```rust
let url = "https://mahjong.chaotic.quest/sthlm-meetups-league/data.json";
```

### 📝 Development Notes

#### Adding New API Endpoints

1. Create handler functions in `src/handlers/`
2. Register routes in `src/routes.rs`
3. Add database operations to `src/db/ankan.rs` as needed

#### Database Schema

Main table structure:
- `meetup_league_player` - Player information
- `meetup_league_table` - Game table information
- `meetup_league_result` - Game results

### 🐛 Troubleshooting

#### Common Issues

1. **Database Connection Failed**
   - Check `DATABASE_URL` environment variable
   - Confirm PostgreSQL service status
   - Verify database permissions

2. **Sync Failed**
   - Check network connection
   - Confirm data source URL accessibility
   - Review logs for specific errors

3. **ID Conflict Errors**
   - Project implements automatic handling mechanism
   - Uses `OVERRIDING SYSTEM VALUE` for ID insertion
   - Supports automatic name updates

### 📈 Performance Features

- Async processing for improved concurrency
- Smart caching to reduce database queries
- Incremental sync to avoid duplicate processing
- Connection pooling for database optimization

### 🤝 Contributing

1. Fork the project
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

### 📄 License

[Add license information here]

### 📞 Contact

[Add contact information here]

---

## 中文

一个用于分析Ankan立直麻将俱乐部比赛数据的Rust服务器应用程序。该项目从远程JSON数据源同步比赛数据，提供RESTful API接口用于查询玩家统计信息和比赛记录。

### 🎯 功能特性

- **数据同步**: 从 `https://mahjong.chaotic.quest/sthlm-meetups-league/data.json` 自动同步比赛数据
- **玩家管理**: 自动维护玩家信息，支持ID一致性检查和姓名更新
- **比赛记录**: 存储和管理多赛季的比赛数据
- **统计分析**: 提供玩家成绩统计和排名功能
- **RESTful API**: 完整的HTTP API接口
- **数据完整性**: 智能处理ID冲突和数据一致性问题

### 🛠 技术栈

- **Rust** - 主要编程语言
- **Axum** - 异步Web框架
- **SQLx** - 异步SQL工具包
- **PostgreSQL** - 数据库
- **Serde** - JSON序列化/反序列化
- **Tokio** - 异步运行时
- **Reqwest** - HTTP客户端

### 📊 数据结构

#### 核心实体

- **LeaguePlayer**: 玩家信息（ID、姓名）
- **LeagueGame**: 比赛信息（赛季、桌号、玩家座位）
- **LeagueResult**: 比赛结果（得分、位次、马点、罚分）

#### 数据源格式

项目支持来自斯德哥尔摩麻将聚会联赛的标准JSON格式：

```json
{
  "collection": {
    "players": [
      {"pid": 0, "name": "Player Name", ...}
    ],
    "games": [
      {
        "gid": 0,
        "played": "2025-01-01",
        "description": "Season 0: Table 1",
        "results": [...]
      }
    ]
  }
}
```

### 🚀 快速开始

#### 环境要求

- Rust 1.70+
- PostgreSQL 12+
- 网络连接（用于数据同步）

#### 安装步骤

1. **克隆项目**
```bash
git clone <repository-url>
cd ankan-meetup-analyser-server
```

2. **配置数据库**
```bash
# 创建数据库
createdb ankan_meetup_league

# 设置数据库URL环境变量
export DATABASE_URL="postgresql://username:password@localhost/ankan_meetup_league"
```

3. **运行数据库迁移**
```bash
sqlx migrate run
```

4. **编译运行**
```bash
cargo run
```

服务器将在 `http://localhost:3000` 启动。

### 📡 API 接口

#### 基础接口

- `GET /` - 服务状态检查
- `GET /hello` - Hello World测试

#### 数据同步

- `GET /sync?force=true` - 强制同步所有数据
- `GET /sync/dry-run` - 预览同步操作（不实际执行）

#### 玩家API

- `GET /api/players` - 获取所有玩家列表
- `GET /api/players/{name}` - 获取指定玩家信息
- `GET /api/players/{name}/matches` - 获取玩家所有比赛记录
- `GET /api/players/{name}/matches/{season}` - 获取玩家指定赛季比赛记录

#### 赛季API

- `GET /api/seasons` - 获取所有赛季列表
- `GET /api/seasons/{season}/players` - 获取指定赛季的玩家列表

### 🔄 数据同步机制

#### 同步流程

1. **玩家数据一致性检查**
   - 检查JSON中的玩家ID是否在数据库中存在
   - 对比相同ID的玩家姓名是否一致
   - 自动更新过时的玩家姓名
   - 创建缺失的玩家记录

2. **比赛数据处理**
   - 解析赛季和桌号信息
   - 处理玩家座位分配（东南西北）
   - 计算和存储比赛结果

3. **数据完整性保证**
   - 使用事务确保数据一致性
   - 智能处理ID冲突
   - 支持增量更新

#### 同步示例

```bash
# 执行完整同步
curl "http://localhost:3000/sync?force=true"

# 预览同步操作
curl "http://localhost:3000/sync/dry-run"
```

### 📁 项目结构

```
src/
├── main.rs              # 应用程序入口点
├── routes.rs            # 路由配置
├── db/
│   ├── mod.rs          # 数据库模块
│   └── ankan.rs        # 数据库操作实现
├── handlers/
│   ├── mod.rs          # 处理器模块
│   ├── sync.rs         # 数据同步处理器
│   └── league_api.rs   # API处理器
└── models/
    ├── mod.rs          # 模型模块
    └── league.rs       # 数据模型定义
```

### 🔧 配置选项

#### 环境变量

- `DATABASE_URL` - PostgreSQL连接字符串
- `PORT` - 服务器端口（默认3000）
- `RUST_LOG` - 日志级别

#### 数据源配置

当前数据源硬编码为斯德哥尔摩麻将联赛：
```rust
let url = "https://mahjong.chaotic.quest/sthlm-meetups-league/data.json";
```

### 📝 开发说明

#### 添加新的API端点

1. 在 `src/handlers/` 中创建处理函数
2. 在 `src/routes.rs` 中注册路由
3. 根据需要添加数据库操作到 `src/db/ankan.rs`

#### 数据库架构

主要表结构：
- `meetup_league_player` - 玩家信息
- `meetup_league_table` - 比赛桌信息
- `meetup_league_result` - 比赛结果

### 🐛 故障排除

#### 常见问题

1. **数据库连接失败**
   - 检查 `DATABASE_URL` 环境变量
   - 确认PostgreSQL服务运行状态
   - 验证数据库权限

2. **同步失败**
   - 检查网络连接
   - 确认数据源URL可访问
   - 查看日志了解具体错误

3. **ID冲突错误**
   - 项目已实现自动处理机制
   - 使用 `OVERRIDING SYSTEM VALUE` 处理ID插入
   - 支持姓名自动更新

### 📈 性能特性

- 异步处理提高并发性能
- 智能缓存减少数据库查询
- 增量同步避免重复处理
- 连接池优化数据库性能

### 🤝 贡献指南

1. Fork项目
2. 创建功能分支
3. 提交更改
4. 推送到分支
5. 创建Pull Request

### 📄 许可证

[在此添加许可证信息]

### 📞 联系方式

[在此添加联系信息]

---

> 此项目专为斯德哥尔摩麻将聚会联赛数据分析而设计，提供完整的比赛数据管理和统计分析功能。
> 
> This project is specifically designed for Stockholm Mahjong Meetup League data analysis, providing complete game data management and statistical analysis features.
