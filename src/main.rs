use sqlx::PgPool;
use crate::db::LeagueRepository;

mod models;
mod handlers;
mod routes;
mod db;

#[tokio::main]
async fn main() {
    // 获取应用路由
    let app = routes::create_router();
    // 在应用程序启动代码中
    let pool = PgPool::connect("postgres://username:password@localhost/database_name").await;
    let league_repo = LeagueRepository::new(pool);

    println!("服务器启动在 0.0.0.0:8080");

    // 运行服务器
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}