use sqlx::{MySql, Pool, MySqlPool};
use anyhow::Result;
use std::env;

pub mod models;
pub mod migrations;

pub type DbPool = Pool<MySql>;

pub struct Database {
    pool: DbPool,
}

impl Database {
    pub async fn new() -> Result<Self> {
        // 加载环境变量
        dotenv::dotenv().ok();

        // 从环境变量构建数据库URL
        let database_url = if let Ok(url) = env::var("DATABASE_URL") {
            url
        } else {
            // 从单独的环境变量构建URL，所有变量都必须设置
            let host = env::var("DB_HOST")
                .map_err(|_| anyhow::anyhow!("DB_HOST 环境变量未设置"))?;
            let port = env::var("DB_PORT")
                .map_err(|_| anyhow::anyhow!("DB_PORT 环境变量未设置"))?;
            let user = env::var("DB_USER")
                .map_err(|_| anyhow::anyhow!("DB_USER 环境变量未设置"))?;
            let password = env::var("DB_PASSWORD")
                .map_err(|_| anyhow::anyhow!("DB_PASSWORD 环境变量未设置"))?;
            let database = env::var("DB_NAME")
                .map_err(|_| anyhow::anyhow!("DB_NAME 环境变量未设置"))?;

            format!("mysql://{}:{}@{}:{}/{}", user, password, host, port, database)
        };

        // 记录连接信息（隐藏密码）
        let safe_url = if let Ok(password) = env::var("DB_PASSWORD") {
            database_url.replace(&password, "***")
        } else {
            // 如果无法获取密码环境变量，简单隐藏整个URL
            "mysql://***:***@***:***/***".to_string()
        };
        tracing::info!("连接数据库: {}", safe_url);

        let pool = MySqlPool::connect(&database_url).await?;

        // 运行数据库迁移
        migrations::run_migrations(&pool).await?;

        Ok(Database { pool })
    }
    
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
    
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
