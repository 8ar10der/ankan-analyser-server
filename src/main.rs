use sqlx::PgPool;
use crate::db::LeagueRepository;
use std::env;
use dotenv::dotenv;

mod models;
mod handlers;
mod routes;
mod db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 加载.env文件中的环境变量
    dotenv().ok();

    // 从环境变量获取数据库URL
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL必须在环境变量中设置");

    // 获取应用路由
    let app = routes::create_router();

    // 连接到数据库
    let pool = PgPool::connect(&database_url).await?;
    let league_repo = LeagueRepository::new(pool.clone());

    // 将仓库实例存储到应用程序状态中
    let app = app.with_state(league_repo);

    println!("服务器启动在 0.0.0.0:8080");

    // 运行服务器
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;

    Ok(())
}