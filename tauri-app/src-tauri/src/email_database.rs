use sqlx::{SqlitePool, Row};
use anyhow::Result;
use dirs;

use crate::database::models::EmailMessage;

pub struct EmailDatabase {
    pool: SqlitePool,
}

impl EmailDatabase {
    // 读取配置的数据库路径
    fn get_configured_database_path() -> Option<std::path::PathBuf> {
        let config_path = dirs::config_dir()?.join("XMail").join("database_config.json");

        if !config_path.exists() {
            return None;
        }

        let config_content = std::fs::read_to_string(&config_path).ok()?;
        let config: serde_json::Value = serde_json::from_str(&config_content).ok()?;

        let db_path_str = config.get("database_path")?.as_str()?;
        let db_path = std::path::PathBuf::from(db_path_str);

        // 验证路径的父目录是否存在
        if let Some(parent) = db_path.parent() {
            if parent.exists() {
                Some(db_path)
            } else {
                tracing::warn!("配置的数据库路径的父目录不存在: {:?}", parent);
                None
            }
        } else {
            None
        }
    }

    pub async fn new() -> Result<Self> {
        // 首先尝试读取自定义配置的数据库路径
        let db_path = Self::get_configured_database_path().unwrap_or_else(|| {
            // 使用系统临时目录或用户数据目录，避免在项目目录中创建数据库文件
            if let Some(data_dir) = dirs::data_dir() {
                let app_data_dir = data_dir.join("XMail");
                // 确保目录存在
                if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
                    tracing::warn!("无法创建应用数据目录 {:?}: {}, 使用临时目录", app_data_dir, e);
                    std::env::temp_dir().join("xmail_emails.db")
                } else {
                    app_data_dir.join("emails.db")
                }
            } else {
                // 如果无法获取数据目录，使用临时目录
                std::env::temp_dir().join("xmail_emails.db")
            }
        });

        // 使用正确的SQLite连接字符串格式
        let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

        tracing::info!("连接SQLite邮件数据库: {}", database_url);
        
        // 创建连接池
        let pool = SqlitePool::connect(&database_url).await?;
        
        let db = Self { pool };
        
        // 运行迁移
        db.run_migrations().await?;
        
        Ok(db)
    }

    async fn run_migrations(&self) -> Result<()> {
        tracing::info!("运行SQLite邮件数据库迁移...");

        // 检查表是否存在
        let table_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='emails'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !table_exists {
            tracing::info!("邮件表不存在，创建新表...");
            // 创建新表，不添加任何唯一性约束，完全按照服务器内容保存
            sqlx::query(
                r#"
                CREATE TABLE emails (
                    id TEXT PRIMARY KEY,
                    account_id TEXT NOT NULL,
                    message_id TEXT NOT NULL,
                    subject TEXT,
                    sender TEXT NOT NULL,
                    recipients TEXT NOT NULL,
                    cc TEXT,
                    bcc TEXT,
                    body_text TEXT,
                    body_html TEXT,
                    folder TEXT NOT NULL,
                    is_read BOOLEAN NOT NULL DEFAULT FALSE,
                    is_starred BOOLEAN NOT NULL DEFAULT FALSE,
                    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
                    received_at TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                )
                "#,
            )
            .execute(&self.pool)
            .await?;
        } else {
            tracing::info!("邮件表已存在，跳过创建");
        }

        // 添加imap_uid字段（如果不存在）
        let add_imap_uid_result = sqlx::query("ALTER TABLE emails ADD COLUMN imap_uid INTEGER")
            .execute(&self.pool)
            .await;

        match add_imap_uid_result {
            Ok(_) => {
                tracing::info!("已添加imap_uid字段到邮件表");
            },
            Err(e) => {
                if e.to_string().contains("duplicate column name") {
                    tracing::info!("imap_uid字段已存在，跳过添加");
                } else {
                    tracing::warn!("添加imap_uid字段失败: {}", e);
                }
            }
        }

        // 创建索引
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_emails_account_folder ON emails(account_id, folder)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_emails_received_at ON emails(received_at DESC)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_emails_is_read ON emails(is_read)")
            .execute(&self.pool)
            .await?;

        // 先清理重复邮件，再创建唯一索引
        if let Err(e) = self.cleanup_duplicate_emails().await {
            tracing::error!("清理重复邮件失败: {}", e);
            // 如果清理失败，删除所有邮件数据重新开始
            tracing::warn!("清理失败，将清空邮件表重新开始");
            sqlx::query("DELETE FROM emails").execute(&self.pool).await?;
        }

        // 创建唯一索引防止重复邮件（基于account_id + message_id）
        let create_unique_index_result = sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_emails_unique ON emails(account_id, message_id)")
            .execute(&self.pool)
            .await;

        if let Err(e) = create_unique_index_result {
            tracing::warn!("创建唯一索引失败，可能存在重复数据: {}", e);
            // 清空表并重新创建索引
            tracing::warn!("将清空邮件表并重新创建索引");
            sqlx::query("DELETE FROM emails").execute(&self.pool).await?;
            sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_emails_unique ON emails(account_id, message_id)")
                .execute(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("重新创建唯一索引失败: {}", e))?;
        }

        tracing::info!("SQLite邮件数据库迁移完成");
        Ok(())
    }

    /// 清理重复邮件
    async fn cleanup_duplicate_emails(&self) -> Result<()> {
        tracing::info!("开始清理重复邮件...");

        // 首先查找重复邮件的数量
        let duplicate_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) - COUNT(DISTINCT account_id || '|' || message_id)
            FROM emails
            "#
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        if duplicate_count == 0 {
            tracing::debug!("没有发现重复邮件");
            return Ok(());
        }

        tracing::info!("发现 {} 条重复邮件，开始清理...", duplicate_count);

        // 使用更简单直接的方式删除重复邮件
        // 直接使用子查询删除重复邮件，保留每组中ID最小的一条
        let result = sqlx::query(
            r#"
            DELETE FROM emails
            WHERE rowid NOT IN (
                SELECT MIN(rowid)
                FROM emails
                GROUP BY account_id, message_id
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        let deleted_count = result.rows_affected();
        if deleted_count > 0 {
            tracing::info!("清理了 {} 条重复邮件", deleted_count);
        }

        Ok(())
    }

    pub async fn save_emails(&self, emails: &[EmailMessage]) -> Result<usize> {
        let mut new_emails_count = 0;
        let mut updated_emails_count = 0;

        tracing::info!("开始保存 {} 封邮件到SQLite数据库", emails.len());

        for (i, email) in emails.iter().enumerate() {
            tracing::info!("处理邮件 {}/{}: {} - {}", i + 1, emails.len(), email.message_id, email.subject);
            // 首先检查邮件是否已存在（基于account_id + message_id的组合，忽略时间差异）
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM emails WHERE account_id = ? AND message_id = ?"
            )
            .bind(&email.account_id)
            .bind(&email.message_id)
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0) > 0;

            if exists {
                // 邮件已存在，更新文件夹信息（处理邮件移动的情况）
                let update_result = sqlx::query(
                    "UPDATE emails SET folder = ?, updated_at = CURRENT_TIMESTAMP WHERE account_id = ? AND message_id = ?"
                )
                .bind(&email.folder)
                .bind(&email.account_id)
                .bind(&email.message_id)
                .execute(&self.pool)
                .await;

                match update_result {
                    Ok(_) => {
                        tracing::info!("邮件文件夹已更新: {} - {} -> {}", email.message_id, email.subject, email.folder);
                        updated_emails_count += 1;
                    },
                    Err(e) => {
                        tracing::error!("更新邮件文件夹失败: {}", e);
                    }
                }
                continue;
            }

            // 插入新邮件
            let result = sqlx::query(
                r#"
                INSERT INTO emails (
                    id, account_id, message_id, subject, sender, recipients, cc, bcc,
                    body_text, body_html, folder, is_read, is_starred, is_deleted,
                    received_at, updated_at, imap_uid
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, ?)
                "#,
            )
            .bind(&email.id)
            .bind(&email.account_id)
            .bind(&email.message_id)
            .bind(&email.subject)
            .bind(&email.sender)
            .bind(&email.recipients)
            .bind(&email.cc)
            .bind(&email.bcc)
            .bind(&email.body_text)
            .bind(&email.body_html)
            .bind(&email.folder)
            .bind(email.is_read)
            .bind(email.is_starred)
            .bind(email.is_deleted)
            .bind(&email.received_at)
            .bind(email.imap_uid)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => {
                    new_emails_count += 1;
                    tracing::debug!("新增邮件: {} - {}", email.message_id, email.subject);
                },
                Err(e) => {
                    tracing::warn!("保存邮件失败 {}: {}", email.id, e);
                }
            }
        }

        tracing::info!("邮件保存完成 - 总计:{}, 新增:{}, 更新:{}",
            emails.len(), new_emails_count, updated_emails_count);

        if new_emails_count > 0 {
            tracing::info!("新增 {} 封邮件到SQLite数据库", new_emails_count);
        }
        if updated_emails_count > 0 {
            tracing::info!("更新 {} 封邮件的文件夹信息", updated_emails_count);
        }
        if new_emails_count == 0 && updated_emails_count == 0 {
            tracing::debug!("没有邮件需要保存或更新");
        }

        Ok(new_emails_count)
    }

    pub async fn get_emails(
        &self,
        account_id: &str,
        folder: &str,
        limit: Option<i64>,
    ) -> Result<Vec<EmailMessage>> {
        // 先检查数据库中的实际情况
        let total_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE account_id = ? AND folder = ?"
        )
        .bind(account_id)
        .bind(folder)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        let deleted_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE account_id = ? AND folder = ? AND is_deleted = TRUE"
        )
        .bind(account_id)
        .bind(folder)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        // 临时修复：统一草稿文件夹名称
        if folder == "Drafts" {
            let update_result = sqlx::query("UPDATE emails SET folder = 'Drafts' WHERE account_id = ? AND folder = 'DRAFTS'")
                .bind(account_id)
                .execute(&self.pool)
                .await;

            if let Ok(result) = update_result {
                if result.rows_affected() > 0 {
                    tracing::info!("统一草稿文件夹名称：将 {} 封邮件从 DRAFTS 更新为 Drafts", result.rows_affected());

                    // 重新统计
                    let new_total_count = sqlx::query_scalar::<_, i64>(
                        "SELECT COUNT(*) FROM emails WHERE account_id = ? AND folder = ?"
                    )
                    .bind(account_id)
                    .bind(folder)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap_or(0);

                    tracing::info!("统一后的数据库统计 - 文件夹: {}, 总邮件: {}, 已删除: {}, 未删除: {}",
                        folder, new_total_count, deleted_count, new_total_count - deleted_count);
                    return self.get_emails_without_stats(account_id, folder, limit).await;
                }
            }
        }

        tracing::info!("数据库统计 - 文件夹: {}, 总邮件: {}, 已删除: {}, 未删除: {}",
            folder, total_count, deleted_count, total_count - deleted_count);

        // 返回所有邮件，不过滤删除状态
        let query = if let Some(limit) = limit {
            sqlx::query(
                r#"
                SELECT * FROM emails
                WHERE account_id = ? AND folder = ?
                ORDER BY received_at DESC
                LIMIT ?
                "#,
            )
            .bind(account_id)
            .bind(folder)
            .bind(limit)
        } else {
            sqlx::query(
                r#"
                SELECT * FROM emails
                WHERE account_id = ? AND folder = ?
                ORDER BY received_at DESC
                "#,
            )
            .bind(account_id)
            .bind(folder)
        };

        let rows = query.fetch_all(&self.pool).await?;
        
        let mut emails = Vec::new();
        for row in rows {
            emails.push(EmailMessage {
                id: row.get("id"),
                account_id: row.get("account_id"),
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                sender: row.get("sender"),
                recipients: row.get("recipients"),
                cc: row.get("cc"),
                bcc: row.get("bcc"),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                folder: row.get("folder"),
                is_read: row.get("is_read"),
                is_starred: row.get("is_starred"),
                is_deleted: row.get("is_deleted"),
                received_at: row.get("received_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                imap_uid: row.get("imap_uid"),
            });
        }
        
        Ok(emails)
    }

    // 不带统计的邮件查询（避免递归调用）
    async fn get_emails_without_stats(
        &self,
        account_id: &str,
        folder: &str,
        limit: Option<i64>,
    ) -> Result<Vec<EmailMessage>> {
        let query = if let Some(limit) = limit {
            sqlx::query(
                r#"
                SELECT * FROM emails
                WHERE account_id = ? AND folder = ?
                ORDER BY received_at DESC
                LIMIT ?
                "#,
            )
            .bind(account_id)
            .bind(folder)
            .bind(limit)
        } else {
            sqlx::query(
                r#"
                SELECT * FROM emails
                WHERE account_id = ? AND folder = ?
                ORDER BY received_at DESC
                "#,
            )
            .bind(account_id)
            .bind(folder)
        };

        let rows = query.fetch_all(&self.pool).await?;

        let mut emails = Vec::new();
        for row in rows {
            emails.push(EmailMessage {
                id: row.get("id"),
                account_id: row.get("account_id"),
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                sender: row.get("sender"),
                recipients: row.get("recipients"),
                cc: row.get("cc"),
                bcc: row.get("bcc"),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                folder: row.get("folder"),
                is_read: row.get("is_read"),
                is_starred: row.get("is_starred"),
                is_deleted: row.get("is_deleted"),
                received_at: row.get("received_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                imap_uid: row.get("imap_uid"),
            });
        }

        Ok(emails)
    }

    pub async fn update_email_status(
        &self,
        account_id: &str,
        message_id: &str,
        is_read: Option<bool>,
        is_starred: Option<bool>,
        is_deleted: Option<bool>,
    ) -> Result<bool> {
        // 简化版本，分别处理不同的更新情况
        let result = if let Some(read) = is_read {
            sqlx::query(
                "UPDATE emails SET is_read = ?, updated_at = CURRENT_TIMESTAMP WHERE account_id = ? AND message_id = ?"
            )
            .bind(read)
            .bind(account_id)
            .bind(message_id)
            .execute(&self.pool)
            .await?
        } else if let Some(starred) = is_starred {
            sqlx::query(
                "UPDATE emails SET is_starred = ?, updated_at = CURRENT_TIMESTAMP WHERE account_id = ? AND message_id = ?"
            )
            .bind(starred)
            .bind(account_id)
            .bind(message_id)
            .execute(&self.pool)
            .await?
        } else if let Some(deleted) = is_deleted {
            sqlx::query(
                "UPDATE emails SET is_deleted = ?, updated_at = CURRENT_TIMESTAMP WHERE account_id = ? AND message_id = ?"
            )
            .bind(deleted)
            .bind(account_id)
            .bind(message_id)
            .execute(&self.pool)
            .await?
        } else {
            return Ok(false);
        };

        Ok(result.rows_affected() > 0)
    }

    pub async fn update_email_read_status(
        &self,
        email_id: &str,
        is_read: bool,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE emails SET is_read = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(is_read)
        .bind(email_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_email(&self, account_id: &str, message_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM emails WHERE account_id = ? AND message_id = ?")
            .bind(account_id)
            .bind(message_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_emails_by_account(&self, account_id: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM emails WHERE account_id = ?")
            .bind(account_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    pub async fn get_email_count(&self, account_id: &str, folder: &str) -> Result<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM emails WHERE account_id = ? AND folder = ? AND is_deleted = FALSE"
        )
        .bind(account_id)
        .bind(folder)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(row.get("count"))
    }

    pub async fn get_unread_count(&self, account_id: &str, folder: &str) -> Result<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM emails WHERE account_id = ? AND folder = ? AND is_read = FALSE AND is_deleted = FALSE"
        )
        .bind(account_id)
        .bind(folder)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(row.get("count"))
    }

    pub async fn delete_emails_by_folder(&self, account_id: &str, folder: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM emails WHERE account_id = ? AND folder = ?")
            .bind(account_id)
            .bind(folder)
            .execute(&self.pool)
            .await?;

        tracing::info!("删除账户 {} 文件夹 {} 的缓存，删除了 {} 条记录", account_id, folder, result.rows_affected());
        Ok(result.rows_affected())
    }

    pub async fn clear_cache(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM emails")
            .execute(&self.pool)
            .await?;

        tracing::info!("清空SQLite邮件缓存，删除了 {} 条记录", result.rows_affected());
        Ok(result.rows_affected())
    }

    /// 获取指定账户的所有收藏邮件
    pub async fn get_starred_emails(&self, account_id: &str) -> Result<Vec<EmailMessage>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM emails
            WHERE account_id = ? AND is_starred = TRUE AND is_deleted = FALSE
            ORDER BY received_at DESC
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;

        let mut emails = Vec::new();
        for row in rows {
            emails.push(EmailMessage {
                id: row.get("id"),
                account_id: row.get("account_id"),
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                sender: row.get("sender"),
                recipients: row.get("recipients"),
                cc: row.get("cc"),
                bcc: row.get("bcc"),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                folder: row.get("folder"),
                is_read: row.get("is_read"),
                is_starred: row.get("is_starred"),
                is_deleted: row.get("is_deleted"),
                received_at: row.get("received_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                imap_uid: row.get("imap_uid"),
            });
        }

        Ok(emails)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
