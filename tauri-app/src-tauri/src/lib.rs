use serde::{Deserialize, Serialize};
use std::sync::{Mutex, Arc};
use std::time::Duration;
use tauri::{State, Manager, Emitter};
use tokio::time;
use sqlx::Row;


mod database;
mod email_service;
mod email_database;
mod oauth2;
mod qq_oauth2_simple;
mod folder_mapper;
mod email_analyzer;
mod gmail_api;
mod outlook_api;
mod smtp_service;

use database::{Database, models::*};
use email_service::EmailService;
use email_database::EmailDatabase;
use folder_mapper::FolderMapper;
use email_analyzer::{EmailAnalyzer, EmailTag};
use oauth2::{OAuth2Client, OAuth2Token, get_gmail_oauth2_config, get_gmail_mail_oauth2_config, get_outlook_oauth2_config, verify_gmail_token, complete_oauth2_flow};

// 应用状态
pub struct AppState {
    pub database: Mutex<Option<Database>>,
    pub email_database: Mutex<Option<Arc<EmailDatabase>>>,
    pub folder_mapper: FolderMapper,
    pub email_analyzer: EmailAnalyzer,
    pub auto_sync_enabled: Mutex<bool>,
    pub sync_interval_minutes: Mutex<u64>,
    pub initialized_accounts: Mutex<std::collections::HashSet<String>>, // 记录已完成初始化的账户
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailProviderConfig {
    pub name: String,
    pub imap_server: String,
    pub imap_port: u16,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub use_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>, // HTML格式的邮件内容
    pub attachments: Vec<String>, // 暂时用字符串表示附件路径
    pub in_reply_to: Option<String>, // 回复的邮件ID
    pub references: Option<String>, // 邮件引用链
    pub draft_id: Option<String>, // 草稿ID（用于更新现有草稿）
    pub is_forward: Option<bool>, // 标识是否为转发邮件
    pub forward_message_id: Option<String>, // 转发的原始邮件ID
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn init_database(state: State<'_, AppState>) -> Result<String, String> {
    // 初始化日志（只初始化一次）
    let _ = tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new("%Y-%m-%d %H:%M:%S".to_string()))
        .try_init();

    // 初始化MySQL数据库（用户账号）
    let database = Database::new().await.map_err(|e| {
        tracing::error!("MySQL数据库初始化失败: {}", e);
        format!("MySQL数据库初始化失败: {}", e)
    })?;

    // 健康检查
    database.health_check().await.map_err(|e| {
        tracing::error!("MySQL数据库健康检查失败: {}", e);
        format!("MySQL数据库连接失败: {}", e)
    })?;

    // 存储MySQL数据库实例到应用状态
    *state.database.lock().unwrap() = Some(database);

    // 初始化SQLite数据库（邮件存储）
    let email_database = EmailDatabase::new().await.map_err(|e| {
        tracing::error!("SQLite邮件数据库初始化失败: {}", e);
        format!("SQLite邮件数据库初始化失败: {}", e)
    })?;

    // 存储SQLite数据库实例到应用状态
    *state.email_database.lock().unwrap() = Some(Arc::new(email_database));

    tracing::info!("数据库初始化成功（MySQL + SQLite）");
    Ok("数据库初始化成功".to_string())
}

#[tauri::command]
async fn create_email_account(
    state: State<'_, AppState>,
    account: NewEmailAccount
) -> Result<EmailAccount, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    let id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();

    // 插入数据库
    sqlx::query(
        r#"
        INSERT INTO email_accounts
        (id, name, email, imap_server, imap_port, smtp_server, smtp_port, username, password, use_tls, created_at, updated_at, is_active, auth_type, access_token, refresh_token, token_expires_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(id.to_string())
    .bind(&account.name)
    .bind(&account.email)
    .bind(&account.imap_server)
    .bind(account.imap_port)
    .bind(&account.smtp_server)
    .bind(account.smtp_port)
    .bind(&account.username)
    .bind(&account.password)
    .bind(account.use_tls)
    .bind(now)
    .bind(now)
    .bind(true)
    .bind(&account.auth_type)
    .bind(&account.access_token)
    .bind(&account.refresh_token)
    .bind(account.token_expires_at)
    .execute(&database)
    .await
    .map_err(|e| {
        tracing::error!("创建邮箱账户失败: {}", e);
        format!("创建邮箱账户失败: {}", e)
    })?;

    // 创建返回的账户对象
    let new_account = EmailAccount {
        id: id.to_string(),
        name: account.name,
        email: account.email,
        imap_server: account.imap_server,
        imap_port: account.imap_port,
        smtp_server: account.smtp_server,
        smtp_port: account.smtp_port,
        username: account.username,
        password: account.password, // 注意：实际应用中应该加密
        use_tls: account.use_tls,
        created_at: now,
        updated_at: now,
        is_active: true,
        auth_type: account.auth_type,
        access_token: account.access_token,
        refresh_token: account.refresh_token,
        token_expires_at: account.token_expires_at,
    };

    tracing::info!("成功创建邮箱账户: {}", new_account.email);
    Ok(new_account)
}

/// 更新OAuth2访问令牌（内部使用，不需要State参数）
pub async fn update_oauth2_token_internal(database: &sqlx::MySqlPool, account_id: &str, new_access_token: &str) -> Result<(), String> {
    sqlx::query(
        "UPDATE email_accounts SET access_token = ? WHERE id = ?"
    )
    .bind(new_access_token)
    .bind(account_id)
    .execute(database)
    .await
    .map_err(|e| {
        tracing::error!("更新OAuth2访问令牌失败: {}", e);
        format!("更新OAuth2访问令牌失败: {}", e)
    })?;

    tracing::info!("OAuth2访问令牌已更新，账户ID: {}", account_id);
    Ok(())
}

#[tauri::command]
async fn save_draft(
    state: State<'_, AppState>,
    account_id: String,
    request: SendEmailRequest,
) -> Result<(), String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| format!("查询账户失败: {}", e))?
    .ok_or("账户不存在或已禁用")?;

    // 根据账户类型使用不同的API保存草稿
    if account.imap_server.contains("outlook") && account.auth_type.as_deref() == Some("oauth2") {
        // 使用Microsoft Graph API保存草稿
        if let Some(access_token) = &account.access_token {
            outlook_api::OutlookApiService::save_draft(access_token, &request).await
                .map_err(|e| format!("保存Outlook草稿失败: {}", e))?;
        } else {
            return Err("Outlook账户缺少访问令牌".to_string());
        }
    } else if account.imap_server.contains("gmail") && account.auth_type.as_deref() == Some("oauth2") {
        // 使用Gmail API保存草稿
        if let Some(access_token) = &account.access_token {
            gmail_api::GmailApiService::save_draft(access_token, &request).await
                .map_err(|e| format!("保存Gmail草稿失败: {}", e))?;
        } else {
            return Err("Gmail账户缺少访问令牌".to_string());
        }
    } else {
        // 对于其他邮件服务（如QQ邮箱），使用IMAP保存草稿
        match crate::email_service::EmailService::save_draft_via_imap(&account, &request).await {
            Ok(_) => {
                tracing::info!("IMAP草稿保存成功");
            },
            Err(e) => {
                tracing::error!("IMAP草稿保存失败: {}", e);
                return Err(format!("保存草稿失败: {}", e));
            }
        }
    }

    tracing::info!("草稿保存成功");
    Ok(())
}

#[tauri::command]
async fn get_email_accounts(state: State<'_, AppState>) -> Result<Vec<EmailAccount>, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    let accounts = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE is_active = TRUE ORDER BY created_at DESC"
    )
    .fetch_all(&database)
    .await
    .map_err(|e| {
        tracing::error!("获取邮箱账户列表失败: {}", e);
        format!("获取邮箱账户列表失败: {}", e)
    })?;

    Ok(accounts)
}

#[tauri::command]
async fn delete_email_account(
    state: State<'_, AppState>,
    account_id: String
) -> Result<(), String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 首先删除SQLite中的邮件数据
    match email_database.delete_emails_by_account(&account_id).await {
        Ok(deleted_count) => {
            tracing::info!("已删除账户 {} 的 {} 封邮件缓存", account_id, deleted_count);
        },
        Err(e) => {
            tracing::warn!("删除账户邮件缓存失败: {}", e);
            // 不阻止账户删除，继续执行
        }
    }

    // 然后删除MySQL中的账户记录
    let result = sqlx::query("DELETE FROM email_accounts WHERE id = ?")
        .bind(&account_id)
        .execute(&database)
        .await
        .map_err(|e| {
            tracing::error!("删除邮箱账户失败: {}", e);
            format!("删除邮箱账户失败: {}", e)
        })?;

    if result.rows_affected() > 0 {
        tracing::info!("成功删除邮箱账户: {}", account_id);
        Ok(())
    } else {
        Err("账户不存在或已被删除".to_string())
    }
}

#[tauri::command]
async fn test_account_connection(_account_id: String) -> Result<bool, String> {
    // 模拟连接测试
    Ok(true)
}

#[tauri::command]
async fn get_database_path() -> Result<String, String> {
    // 读取保存的数据库路径配置
    if let Some(data_dir) = dirs::data_dir() {
        let config_dir = data_dir.join("XMail");
        let config_file = config_dir.join("database_config.json");

        if config_file.exists() {
            if let Ok(config_content) = std::fs::read_to_string(&config_file) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&config_content) {
                    if let Some(path) = config.get("database_path").and_then(|p| p.as_str()) {
                        return Ok(path.to_string());
                    }
                }
            }
        }
    }

    // 如果没有配置文件，返回默认路径
    let db_path = if let Some(data_dir) = dirs::data_dir() {
        let app_data_dir = data_dir.join("XMail");
        app_data_dir.join("emails.db")
    } else {
        std::env::temp_dir().join("xmail_emails.db")
    };

    Ok(db_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn select_database_path() -> Result<Option<String>, String> {
    // 在Tauri v2中，文件对话框应该在前端处理
    // 这个命令只是一个占位符，实际的文件选择在前端完成
    Ok(None)
}

#[tauri::command]
#[allow(non_snake_case)]
async fn set_database_path(path: String, migrateData: bool) -> Result<(), String> {
    let migrate_data = migrateData;
    // 验证路径
    let path_buf = std::path::PathBuf::from(&path);
    let parent_dir = path_buf.parent().ok_or("无效的路径")?;

    if !parent_dir.exists() {
        return Err("指定的目录不存在".to_string());
    }

    // 如果需要迁移数据，先复制现有数据库
    if migrate_data {
        // 获取当前数据库路径
        let current_db_path = if let Some(data_dir) = dirs::data_dir() {
            let app_data_dir = data_dir.join("XMail");
            app_data_dir.join("emails.db")
        } else {
            std::env::temp_dir().join("xmail_emails.db")
        };

        // 如果当前数据库存在，复制到新位置
        if current_db_path.exists() {
            // 确保新路径的目录存在
            std::fs::create_dir_all(parent_dir)
                .map_err(|e| format!("创建目标目录失败: {}", e))?;

            // 复制数据库文件
            std::fs::copy(&current_db_path, &path_buf)
                .map_err(|e| format!("复制数据库文件失败: {}", e))?;

            tracing::info!("数据库文件已从 {:?} 复制到 {:?}", current_db_path, path_buf);
        } else {
            tracing::info!("当前数据库文件不存在，将在新位置创建空数据库");
        }
    }

    // 保存配置到文件
    let config_path = if let Some(config_dir) = dirs::config_dir() {
        let app_config_dir = config_dir.join("XMail");
        std::fs::create_dir_all(&app_config_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
        app_config_dir.join("database_config.json")
    } else {
        return Err("无法获取配置目录".to_string());
    };

    let config = serde_json::json!({
        "database_path": path
    });

    std::fs::write(&config_path, config.to_string())
        .map_err(|e| format!("保存配置失败: {}", e))?;

    tracing::info!("数据库路径配置已保存: {}", path);
    Ok(())
}

#[tauri::command]
async fn reset_database_path() -> Result<(), String> {
    // 删除配置文件，恢复默认路径
    let config_path = if let Some(config_dir) = dirs::config_dir() {
        let app_config_dir = config_dir.join("XMail");
        app_config_dir.join("database_config.json")
    } else {
        return Err("无法获取配置目录".to_string());
    };

    if config_path.exists() {
        std::fs::remove_file(&config_path)
            .map_err(|e| format!("删除配置文件失败: {}", e))?;
        tracing::info!("数据库路径配置已重置为默认");
    }

    Ok(())
}

#[tauri::command]
async fn test_email_connection(account: NewEmailAccount) -> Result<bool, String> {
    // 直接测试连接，不保存到数据库

    // 创建临时账户对象用于测试
    let temp_account = crate::database::models::EmailAccount {
        id: uuid::Uuid::new_v4().to_string(),
        name: account.name,
        email: account.email,
        imap_server: account.imap_server,
        imap_port: account.imap_port,
        smtp_server: account.smtp_server,
        smtp_port: account.smtp_port,
        username: account.username,
        password: account.password,
        use_tls: account.use_tls,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_active: true,
        auth_type: account.auth_type,
        access_token: account.access_token,
        refresh_token: account.refresh_token,
        token_expires_at: account.token_expires_at,
    };

    // 测试连接
    match EmailService::test_connection(&temp_account).await {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::error!("连接测试失败: {}", e);
            Err(format!("连接测试失败: {}", e))
        }
    }
}

#[tauri::command]
async fn get_messages(
    state: State<'_, AppState>,
    account_id: String,
    folder: Option<String>,
    limit: Option<i64>,
    force_refresh: Option<bool>,
) -> Result<Vec<EmailMessage>, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
    };

    // 克隆EmailDatabase以避免借用问题
    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询账户失败: {}", e);
        format!("查询账户失败: {}", e)
    })?
    .ok_or("账户不存在或已禁用")?;

    let standard_folder = folder.unwrap_or_else(|| "INBOX".to_string());
    let force_refresh = force_refresh.unwrap_or(false);

    // 调试信息：记录调用来源
    tracing::info!("get_messages 调用 - 账户: {}, 文件夹: {}, 强制刷新: {}", account_id, standard_folder, force_refresh);

    // 映射文件夹名称到服务器实际的文件夹名称
    let actual_folder = match state.folder_mapper.map_folder(&account, &standard_folder).await {
        Ok(folder) => folder,
        Err(e) => {
            tracing::error!("文件夹映射失败: {}", e);
            return Err(format!("文件夹映射失败: {}", e));
        }
    };

    // 如果强制刷新或SQLite中没有数据，从服务器获取
    if force_refresh {
        // 强制刷新时，暂停实时监听并清空该账户的邮件缓存
        tracing::info!("强制刷新：暂停实时监听");
        *state.auto_sync_enabled.lock().unwrap() = false;

        // 强制刷新时清除初始化状态
        {
            let mut initialized = state.initialized_accounts.lock().unwrap();
            initialized.remove(&account_id);
            tracing::info!("强制刷新：清除账户 {} 的初始化状态", account_id);
        }

        tracing::info!("强制刷新：清空账户 {} 的邮件缓存", account_id);
        if let Err(e) = email_database.delete_emails_by_account(&account_id).await {
            tracing::warn!("清空账户邮件缓存失败: {}", e);
        }

        tracing::info!("从服务器获取邮件...");

        // 检查账户类型，使用对应的API
        let messages = if account.imap_server.contains("gmail") && account.auth_type.as_deref() == Some("oauth2") {
            tracing::debug!("使用Gmail API获取邮件");
            if let Some(access_token) = &account.access_token {
                match gmail_api::GmailApiService::fetch_messages(access_token, &actual_folder, limit).await {
                    Ok(mut messages) => {
                        // 设置account_id
                        for message in &mut messages {
                            message.account_id = account_id.clone();
                        }
                        messages
                    },
                    Err(e) => {
                        let error_str = e.to_string();
                        tracing::debug!("Gmail API错误详情: {}", error_str);

                        // 如果是401错误，尝试刷新令牌
                        if error_str.contains("401") || error_str.contains("Unauthorized") {
                            tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");
                            tracing::info!("账户 {} 的刷新令牌状态: {}", account_id,
                                if account.refresh_token.is_some() { "存在" } else { "不存在" });

                            if let Some(refresh_token) = &account.refresh_token {
                                // 根据账户类型选择正确的OAuth2配置和刷新方法
                                let refresh_result = if account.imap_server.contains("outlook") || account.imap_server.contains("office365") {
                                    // Outlook账户
                                    let config = get_outlook_oauth2_config();
                                    let client = OAuth2Client::new(config);
                                    client.refresh_outlook_token(refresh_token).await
                                } else {
                                    // Gmail账户
                                    let config = get_gmail_mail_oauth2_config();
                                    let client = OAuth2Client::new(config);
                                    client.refresh_token(refresh_token).await
                                };

                                match refresh_result {
                                    Ok(new_token) => {
                                        tracing::info!("OAuth2令牌刷新成功");

                                        // 更新数据库中的访问令牌
                                        if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                            tracing::error!("更新访问令牌失败: {}", e);
                                            return Err(format!("更新访问令牌失败: {}", e));
                                        }

                                        // 使用新令牌重试
                                        match gmail_api::GmailApiService::fetch_messages(&new_token.access_token, &actual_folder, limit).await {
                                            Ok(mut messages) => {
                                                tracing::info!("使用新令牌成功获取 {} 封邮件", messages.len());
                                                // 设置account_id
                                                for message in &mut messages {
                                                    message.account_id = account_id.clone();
                                                }
                                                messages
                                            },
                                            Err(e) => {
                                                tracing::error!("使用新令牌仍然失败: {}", e);
                                                return Err(format!("使用新令牌仍然失败: {}", e));
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!("OAuth2令牌刷新失败: {}", e);
                                        return Err(format!("OAuth2令牌刷新失败: {}", e));
                                    }
                                }
                            } else {
                                tracing::error!("没有刷新令牌，无法刷新访问令牌");
                                return Err("没有刷新令牌，无法刷新访问令牌".to_string());
                            }
                        } else {
                            tracing::error!("Gmail API获取邮件失败（非401错误）: {}", e);
                            return Err(format!("Gmail API获取邮件失败: {}", e));
                        }
                    }
                }
            } else {
                return Err("Gmail账户缺少访问令牌".to_string());
            }
        } else if (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) && account.auth_type.as_deref() == Some("oauth2") {
            // 使用Outlook Graph API
            tracing::debug!("使用Outlook Graph API获取邮件");
            if let Some(access_token) = &account.access_token {
                match outlook_api::OutlookApiService::fetch_messages(access_token, &actual_folder, limit).await {
                    Ok(mut messages) => {
                        // 设置account_id
                        for message in &mut messages {
                            message.account_id = account_id.clone();
                        }
                        messages
                    },
                    Err(e) => {
                        let error_str = e.to_string();
                        tracing::debug!("Outlook Graph API错误详情: {}", error_str);

                        // 如果是401错误，尝试刷新令牌
                        if error_str.contains("401") || error_str.contains("Unauthorized") {
                            tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");
                            tracing::info!("账户 {} 的刷新令牌状态: {}", account_id,
                                if account.refresh_token.is_some() { "存在" } else { "不存在" });

                            if let Some(refresh_token) = &account.refresh_token {
                                // 刷新Outlook令牌
                                let config = get_outlook_oauth2_config();
                                let client = OAuth2Client::new(config);
                                match client.refresh_outlook_token(refresh_token).await {
                                    Ok(new_token) => {
                                        tracing::info!("成功刷新Outlook访问令牌");

                                        // 更新数据库中的令牌
                                        if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                            tracing::error!("更新数据库中的令牌失败: {}", e);
                                        }

                                        // 使用新令牌重试
                                        match outlook_api::OutlookApiService::fetch_messages(&new_token.access_token, &actual_folder, limit).await {
                                            Ok(mut messages) => {
                                                for message in &mut messages {
                                                    message.account_id = account_id.clone();
                                                }
                                                messages
                                            },
                                            Err(retry_error) => {
                                                tracing::error!("使用新令牌重试Outlook Graph API失败: {}", retry_error);
                                                return Err(format!("Outlook Graph API获取邮件失败: {}", retry_error));
                                            }
                                        }
                                    },
                                    Err(refresh_error) => {
                                        tracing::error!("刷新Outlook访问令牌失败: {}", refresh_error);
                                        return Err(format!("刷新Outlook访问令牌失败: {}", refresh_error));
                                    }
                                }
                            } else {
                                tracing::error!("没有刷新令牌，无法刷新访问令牌");
                                return Err("没有刷新令牌，无法刷新访问令牌".to_string());
                            }
                        } else {
                            tracing::error!("Outlook Graph API获取邮件失败（非401错误）: {}", e);
                            return Err(format!("Outlook Graph API获取邮件失败: {}", e));
                        }
                    }
                }
            } else {
                return Err("Outlook账户缺少访问令牌".to_string());
            }
        } else {
            // 使用传统IMAP方式
            match EmailService::fetch_messages_with_db(&account, &actual_folder, limit, Some(&database)).await {
                Ok(messages) => messages,
                Err(e) => {
                    tracing::error!("IMAP获取邮件失败: {}", e);
                    return Err(format!("IMAP获取邮件失败: {}", e));
                }
            }
        };

        // 保存邮件到SQLite数据库
        if !messages.is_empty() {
            // 保存到SQLite数据库
            if let Err(e) = email_database.save_emails(&messages).await {
                tracing::warn!("保存邮件到SQLite失败: {}", e);
            }
            tracing::info!("成功获取 {} 封邮件", messages.len());

            // 强制刷新完成，重新启动实时监听
            tracing::info!("强制刷新完成，重新启动实时监听");

            // 重新启动实时监听
            {
                let mut sync_enabled = state.auto_sync_enabled.lock().unwrap();
                if !*sync_enabled {
                    *sync_enabled = true;
                    tracing::info!("强制刷新完成，重新启动实时邮件监听");
                }
            }

            // 重要：从数据库重新读取邮件，确保返回正确的用户状态（已读/未读等）
            match email_database.get_emails(&account_id, &actual_folder, limit).await {
                Ok(db_messages) => {
                    tracing::info!("从数据库返回 {} 封邮件（包含正确的用户状态）", db_messages.len());
                    Ok(db_messages)
                },
                Err(e) => {
                    tracing::warn!("从数据库读取邮件失败: {}，返回服务器数据", e);
                    Ok(messages) // 降级到服务器数据
                }
            }
        } else {
            Ok(vec![])
        }
    } else {
        // 先尝试从SQLite获取（使用映射后的文件夹名）
        match email_database.get_emails(&account_id, &actual_folder, limit).await {
            Ok(cached_messages) if !cached_messages.is_empty() => {
                tracing::info!("使用缓存邮件，共 {} 封", cached_messages.len());

                // 检查是否是首次加载（基于初始化状态）
                let is_first_load = {
                    let initialized = state.initialized_accounts.lock().unwrap();
                    !initialized.contains(&account_id)
                };

                if is_first_load {
                    tracing::info!("账户 {} 首次使用缓存加载完成", account_id);

                    // 标记账户已初始化
                    {
                        let mut initialized = state.initialized_accounts.lock().unwrap();
                        initialized.insert(account_id.clone());
                    }

                    // 启动实时监听（如果尚未启动）
                    let mut sync_enabled = state.auto_sync_enabled.lock().unwrap();
                    if !*sync_enabled {
                        *sync_enabled = true;
                        tracing::info!("启动实时邮件监听");
                    }
                } else {
                    tracing::info!("账户 {} 使用缓存（非首次加载）", account_id);
                }

                Ok(cached_messages)
            },
            _ => {
                // SQLite中没有数据，从服务器获取
                tracing::info!("从服务器获取邮件...");

                // 检查账户类型，使用对应的API
                let messages = if account.imap_server.contains("gmail") && account.auth_type.as_deref() == Some("oauth2") {
                    tracing::debug!("使用Gmail API获取邮件");
                    if let Some(access_token) = &account.access_token {
                        match gmail_api::GmailApiService::fetch_messages(access_token, &actual_folder, limit).await {
                            Ok(mut messages) => {
                                // 设置account_id
                                for message in &mut messages {
                                    message.account_id = account_id.clone();
                                }
                                messages
                            },
                            Err(e) => {
                                let error_str = e.to_string();
                                tracing::debug!("Gmail API错误详情: {}", error_str);

                                // 如果是401错误，尝试刷新令牌
                                if error_str.contains("401") || error_str.contains("Unauthorized") {
                                    tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");
                                    tracing::info!("账户 {} 的刷新令牌状态: {}", account_id,
                                        if account.refresh_token.is_some() { "存在" } else { "不存在" });

                                    if let Some(refresh_token) = &account.refresh_token {
                                        // 根据账户类型选择正确的OAuth2配置和刷新方法
                                        let refresh_result = if account.imap_server.contains("outlook") || account.imap_server.contains("office365") {
                                            // Outlook账户
                                            let config = get_outlook_oauth2_config();
                                            let client = OAuth2Client::new(config);
                                            client.refresh_outlook_token(refresh_token).await
                                        } else {
                                            // Gmail账户
                                            let config = get_gmail_mail_oauth2_config();
                                            let client = OAuth2Client::new(config);
                                            client.refresh_token(refresh_token).await
                                        };

                                        match refresh_result {
                                            Ok(new_token) => {
                                                tracing::info!("OAuth2令牌刷新成功");

                                                // 更新数据库中的访问令牌
                                                if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                                    tracing::error!("更新访问令牌失败: {}", e);
                                                    return Err(format!("更新访问令牌失败: {}", e));
                                                }

                                                // 使用新令牌重试
                                                match gmail_api::GmailApiService::fetch_messages(&new_token.access_token, &actual_folder, limit).await {
                                                    Ok(mut messages) => {
                                                        tracing::info!("使用新令牌成功获取 {} 封邮件", messages.len());
                                                        // 设置account_id
                                                        for message in &mut messages {
                                                            message.account_id = account_id.clone();
                                                        }
                                                        messages
                                                    },
                                                    Err(e) => {
                                                        tracing::error!("使用新令牌仍然失败: {}", e);
                                                        return Err(format!("使用新令牌仍然失败: {}", e));
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                tracing::error!("OAuth2令牌刷新失败: {}", e);
                                                return Err(format!("OAuth2令牌刷新失败: {}", e));
                                            }
                                        }
                                    } else {
                                        tracing::error!("没有刷新令牌，无法刷新访问令牌");
                                        return Err("没有刷新令牌，无法刷新访问令牌".to_string());
                                    }
                                } else {
                                    tracing::error!("Gmail API获取邮件失败（非401错误）: {}", e);
                                    return Err(format!("Gmail API获取邮件失败: {}", e));
                                }
                            }
                        }
                    } else {
                        return Err("Gmail账户缺少访问令牌".to_string());
                    }
                } else if (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) && account.auth_type.as_deref() == Some("oauth2") {
                    // 使用Outlook Graph API
                    tracing::debug!("使用Outlook Graph API获取邮件");
                    if let Some(access_token) = &account.access_token {
                        match outlook_api::OutlookApiService::fetch_messages(access_token, &actual_folder, limit).await {
                            Ok(mut messages) => {
                                // 设置account_id
                                for message in &mut messages {
                                    message.account_id = account_id.clone();
                                }
                                messages
                            },
                            Err(e) => {
                                let error_str = e.to_string();
                                tracing::debug!("Outlook Graph API错误详情: {}", error_str);

                                // 如果是401错误，尝试刷新令牌
                                if error_str.contains("401") || error_str.contains("Unauthorized") {
                                    tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");

                                    if let Some(refresh_token) = &account.refresh_token {
                                        // 刷新Outlook令牌
                                        let config = get_outlook_oauth2_config();
                                        let client = OAuth2Client::new(config);
                                        match client.refresh_outlook_token(refresh_token).await {
                                            Ok(new_token) => {
                                                tracing::info!("成功刷新Outlook访问令牌");

                                                // 更新数据库中的令牌
                                                if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                                    tracing::error!("更新数据库中的令牌失败: {}", e);
                                                }

                                                // 使用新令牌重试
                                                match outlook_api::OutlookApiService::fetch_messages(&new_token.access_token, &actual_folder, limit).await {
                                                    Ok(mut messages) => {
                                                        for message in &mut messages {
                                                            message.account_id = account_id.clone();
                                                        }
                                                        messages
                                                    },
                                                    Err(retry_error) => {
                                                        tracing::error!("使用新令牌重试Outlook Graph API失败: {}", retry_error);
                                                        return Err(format!("Outlook Graph API获取邮件失败: {}", retry_error));
                                                    }
                                                }
                                            },
                                            Err(refresh_error) => {
                                                tracing::error!("刷新Outlook访问令牌失败: {}", refresh_error);
                                                return Err(format!("刷新Outlook访问令牌失败: {}", refresh_error));
                                            }
                                        }
                                    } else {
                                        tracing::error!("没有刷新令牌，无法刷新访问令牌");
                                        return Err("没有刷新令牌，无法刷新访问令牌".to_string());
                                    }
                                } else {
                                    tracing::error!("Outlook Graph API获取邮件失败（非401错误）: {}", e);
                                    return Err(format!("Outlook Graph API获取邮件失败: {}", e));
                                }
                            }
                        }
                    } else {
                        return Err("Outlook账户缺少访问令牌".to_string());
                    }
                } else {
                    // 使用传统IMAP方式
                    match EmailService::fetch_messages_with_db(&account, &actual_folder, limit, Some(&database)).await {
                        Ok(messages) => messages,
                        Err(e) => {
                            tracing::error!("IMAP获取邮件失败: {}", e);
                            return Err(format!("IMAP获取邮件失败: {}", e));
                        }
                    }
                };

                // 保存到SQLite数据库
                if let Err(e) = email_database.save_emails(&messages).await {
                    tracing::warn!("保存邮件到SQLite失败: {}", e);
                }
                tracing::info!("成功获取 {} 封邮件", messages.len());

                // 从数据库重新读取邮件，确保返回正确的用户状态
                let result = match email_database.get_emails(&account_id, &actual_folder, limit).await {
                    Ok(db_messages) => {
                        tracing::debug!("从数据库返回 {} 封邮件", db_messages.len());
                        Ok(db_messages)
                    },
                    Err(e) => {
                        tracing::warn!("从数据库读取邮件失败: {}，返回服务器数据", e);
                        Ok(messages) // 降级到服务器数据
                    }
                };

                // 标记该账户已完成初始化
                {
                    let mut initialized = state.initialized_accounts.lock().unwrap();
                    initialized.insert(account_id.clone());
                    tracing::info!("账户 {} 完成初始化，该账户现在可以进行实时监听", account_id);
                }

                // 启动实时监听（如果尚未启动）
                let mut sync_enabled = state.auto_sync_enabled.lock().unwrap();
                if !*sync_enabled {
                    *sync_enabled = true;
                    if force_refresh {
                        tracing::info!("强制刷新完成，重新启动实时邮件监听");
                    } else {
                        tracing::info!("启动实时邮件监听");
                    }
                }

                result
            }
        }
    }
}



#[tauri::command]
async fn send_email(
    state: State<'_, AppState>,
    account_id: String,
    request: serde_json::Value,
) -> Result<(), String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| format!("查询账户失败: {}", e))?
    .ok_or("账户不存在或已禁用")?;

    // 解析发送请求
    let send_request: SendEmailRequest = serde_json::from_value(request)
        .map_err(|e| format!("解析发送请求失败: {}", e))?;

    // 调用SMTP发送服务
    match smtp_service::SmtpService::send_email(&account, &send_request).await {
        Ok(()) => {
            // 邮件发送成功后，立即刷新已发送文件夹缓存
            tracing::info!("邮件发送成功，开始刷新已发送文件夹缓存");

            let email_database = {
                let guard = state.email_database.lock().unwrap();
                guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
            };

            // 只有传统IMAP邮箱（非Outlook、非Gmail）才需要IMAP刷新已发送文件夹
            let is_outlook = account.email.contains("@outlook.com") || account.email.contains("@hotmail.com") || account.email.contains("@live.com");
            let is_gmail = account.email.contains("@gmail.com");

            if !is_outlook && !is_gmail {
                // 使用与SMTP保存相同的文件夹查找逻辑
                let actual_folder = match email_service::EmailService::find_sent_folder(&account).await {
                    Ok(folder) => folder,
                    Err(e) => {
                        tracing::warn!("查找已发送文件夹失败，使用文件夹映射: {}", e);
                        match state.folder_mapper.map_folder(&account, "SENT").await {
                            Ok(folder) => folder,
                            Err(e2) => {
                                tracing::warn!("文件夹映射也失败，使用默认值: {}", e2);
                                "SENT".to_string()
                            }
                        }
                    }
                };

                tracing::info!("准备刷新已发送文件夹（非Outlook），映射后的文件夹名: {}", actual_folder);

                // 立即刷新已发送文件夹缓存（不阻塞响应）
                let account_clone = account.clone();
                let actual_folder_clone = actual_folder.clone();
                tokio::spawn(async move {
                    tracing::info!("后台任务：立即刷新已发送文件夹: {}", actual_folder_clone);

                    // 直接重新获取已发送文件夹的邮件
                    match email_service::EmailService::fetch_messages(&account_clone, &actual_folder_clone, None).await {
                        Ok(messages) => {
                            // 调试信息：显示邮件的文件夹信息
                            if let Some(first_message) = messages.first() {
                                tracing::info!("后台任务获取的邮件文件夹: {}", first_message.folder);
                            }

                            // 保存到SQLite数据库（会自动处理重复邮件）
                            if let Err(e) = email_database.save_emails(&messages).await {
                                tracing::warn!("保存已发送邮件到SQLite失败: {}", e);
                            } else {
                                tracing::info!("已发送文件夹缓存已更新，共 {} 封邮件，文件夹: {}", messages.len(), actual_folder_clone);

                            // 发送缓存更新完成事件给前端
                            // TODO: 这里可以添加事件发送逻辑
                            }
                        },
                        Err(e) => {
                            tracing::warn!("刷新已发送文件夹失败: {}", e);
                        }
                    }
                });
            } else if is_outlook {
                tracing::info!("Outlook邮箱发送成功，使用Graph API，无需IMAP刷新");
            } else if is_gmail {
                tracing::info!("Gmail邮箱发送成功，使用Gmail API，无需IMAP刷新");
            }

            Ok(())
        },
        Err(e) => {
            let error_str = e.to_string();
            tracing::info!("邮件发送错误详情: {}", error_str);

            // 如果是401错误，尝试刷新令牌
            if error_str.contains("401") || error_str.contains("Unauthorized") || error_str.contains("InvalidAuthenticationToken") {
                tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");

                if let Some(refresh_token) = &account.refresh_token {
                    // 根据账户类型选择正确的OAuth2配置和刷新方法
                    let refresh_result = if account.imap_server.contains("outlook") || account.imap_server.contains("office365") {
                        // Outlook账户
                        let config = get_outlook_oauth2_config();
                        let client = OAuth2Client::new(config);
                        client.refresh_outlook_token(refresh_token).await
                    } else {
                        // Gmail账户
                        let config = get_gmail_oauth2_config();
                        let client = OAuth2Client::new(config);
                        client.refresh_token(refresh_token).await
                    };

                    match refresh_result {
                        Ok(new_token) => {
                            tracing::info!("成功刷新访问令牌");

                            // 更新数据库中的令牌
                            if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                tracing::error!("更新数据库中的令牌失败: {}", e);
                            }

                            // 更新账户对象中的令牌
                            let mut updated_account = account;
                            updated_account.access_token = Some(new_token.access_token);

                            // 使用新令牌重试发送邮件
                            match smtp_service::SmtpService::send_email(&updated_account, &send_request).await {
                                Ok(()) => {
                                    tracing::info!("使用新令牌成功发送邮件");
                                    Ok(())
                                },
                                Err(retry_e) => {
                                    tracing::error!("使用新令牌发送邮件仍然失败: {}", retry_e);
                                    Err(format!("发送邮件失败: {}", retry_e))
                                }
                            }
                        },
                        Err(refresh_e) => {
                            tracing::error!("刷新令牌失败: {}", refresh_e);
                            Err(format!("发送邮件失败，令牌刷新也失败: {}", refresh_e))
                        }
                    }
                } else {
                    tracing::error!("账户没有刷新令牌，无法自动刷新");
                    Err(format!("发送邮件失败: {}", error_str))
                }
            } else {
                Err(format!("发送邮件失败: {}", error_str))
            }
        }
    }
}

#[tauri::command]
async fn mark_message_as_read(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
    is_read: bool,
) -> Result<(), String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询账户失败: {}", e);
        format!("查询账户失败: {}", e)
    })?
    .ok_or("账户不存在或已禁用")?;

    // 根据账户类型选择标记方式
    let is_gmail = account.email.ends_with("@gmail.com") ||
                   account.imap_server.contains("gmail");

    let is_outlook = (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) &&
                     account.auth_type.as_deref() == Some("oauth2");

    // 获取真实的邮件ID和元数据（用于API调用和IMAP操作）
    let (real_message_id, email_subject, email_sender, email_date) = {
        // 通过id查找邮件记录，获取邮件信息用于IMAP操作
        tracing::debug!("查询邮件记录: account_id={}, id={}", account_id, message_id);
        match sqlx::query("SELECT message_id, subject, sender, received_at FROM emails WHERE account_id = ? AND id = ?")
            .bind(&account_id)
            .bind(&message_id)
            .fetch_optional(email_database.pool())
            .await
        {
            Ok(Some(row)) => {
                let real_id: String = row.get("message_id");
                let subject: String = row.get("subject");
                let sender: String = row.get("sender");
                let received_at: chrono::DateTime<chrono::Utc> = row.get("received_at");

                tracing::info!("找到真实邮件ID: {} -> {}", message_id, real_id);
                (real_id, Some(subject), Some(sender), Some(received_at.format("%Y-%m-%d").to_string()))
            },
            Ok(None) => {
                tracing::error!("未找到要标记的邮件记录: {}", message_id);
                return Err("未找到要标记的邮件".to_string());
            },
            Err(e) => {
                tracing::error!("查询邮件记录失败: {}", e);
                return Err(format!("查询邮件记录失败: {}", e));
            }
        }
    };

    // 先尝试在服务器上标记已读状态
    if is_gmail && account.access_token.is_some() {
        // Gmail账户使用Gmail API标记
        match crate::gmail_api::GmailApiService::mark_as_read(&account.access_token.unwrap_or_default(), &real_message_id, is_read).await {
            Ok(_) => {
                tracing::info!("Gmail服务器已标记邮件为{}: {}", if is_read { "已读" } else { "未读" }, real_message_id);
            },
            Err(e) => {
                tracing::warn!("Gmail API标记邮件已读状态失败: {}", e);
                // 不阻止本地更新，继续执行
            }
        }
    } else if is_outlook && account.access_token.is_some() {
        // Outlook账户使用Graph API标记
        match crate::outlook_api::OutlookApiService::mark_as_read(&account.access_token.unwrap_or_default(), &real_message_id, is_read).await {
            Ok(_) => {
                tracing::info!("Outlook服务器已标记邮件为{}: {}", if is_read { "已读" } else { "未读" }, real_message_id);
            },
            Err(e) => {
                let error_str = e.to_string();
                tracing::debug!("Outlook Graph API标记已读错误详情: {}", error_str);

                // 如果是401错误，尝试刷新令牌
                if error_str.contains("401") || error_str.contains("Unauthorized") || error_str.contains("InvalidAuthenticationToken") {
                    tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");

                    if let Some(refresh_token) = &account.refresh_token {
                        // 刷新Outlook令牌
                        let config = get_outlook_oauth2_config();
                        let client = OAuth2Client::new(config);
                        match client.refresh_outlook_token(refresh_token).await {
                            Ok(new_token) => {
                                tracing::info!("成功刷新Outlook访问令牌");

                                // 更新数据库中的令牌
                                if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                    tracing::error!("更新数据库中的令牌失败: {}", e);
                                }

                                // 使用新令牌重试标记已读
                                match crate::outlook_api::OutlookApiService::mark_as_read(&new_token.access_token, &real_message_id, is_read).await {
                                    Ok(_) => {
                                        tracing::info!("使用新令牌成功标记邮件为{}: {}", if is_read { "已读" } else { "未读" }, real_message_id);
                                    },
                                    Err(retry_e) => {
                                        tracing::warn!("使用新令牌标记邮件已读状态仍然失败: {}", retry_e);
                                    }
                                }
                            },
                            Err(refresh_e) => {
                                tracing::error!("刷新Outlook令牌失败: {}", refresh_e);
                            }
                        }
                    } else {
                        tracing::error!("没有刷新令牌，无法刷新访问令牌");
                    }
                } else {
                    tracing::warn!("Outlook Graph API标记邮件已读状态失败: {}", e);
                }
            }
        }
    } else {
        // QQ邮箱等其他邮箱使用IMAP STORE命令标记已读状态
        tracing::info!("使用IMAP STORE命令标记邮件已读状态（账户: {}）", account.email);
        match crate::email_service::EmailService::mark_as_read_via_imap_with_fallback(
            &account,
            &real_message_id,
            is_read,
            email_subject.as_deref(),
            email_sender.as_deref(),
            email_date.as_deref()
        ).await {
            Ok(_) => {
                tracing::info!("IMAP服务器已标记邮件为{}: {}", if is_read { "已读" } else { "未读" }, real_message_id);
            },
            Err(e) => {
                tracing::warn!("IMAP标记邮件已读状态失败: {}，仅更新本地缓存", e);
                // 不阻止本地更新，继续执行
            }
        }
    }

    // 更新本地SQLite数据库中的邮件已读状态
    match email_database.update_email_read_status(&message_id, is_read).await {
        Ok(updated) => {
            if updated {
                tracing::info!("本地缓存已标记邮件为{}: {}", if is_read { "已读" } else { "未读" }, message_id);
                Ok(())
            } else {
                Err("邮件不存在或更新失败".to_string())
            }
        },
        Err(e) => {
            tracing::error!("更新本地邮件已读状态失败: {}", e);
            Err(format!("更新本地邮件已读状态失败: {}", e))
        }
    }
}

#[tauri::command]
async fn mark_message_as_starred(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
    is_starred: bool,
) -> Result<(), String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询账户失败: {}", e);
        format!("查询账户失败: {}", e)
    })?
    .ok_or("账户不存在或已禁用")?;

    // 获取真实的邮件ID（用于API调用）
    let real_message_id = {
        // 通过message_id查找邮件记录，获取真实的邮件ID（存储在message_id字段中）
        match sqlx::query("SELECT message_id FROM emails WHERE account_id = ? AND id = ?")
            .bind(&account_id)
            .bind(&message_id)
            .fetch_optional(email_database.pool())
            .await
        {
            Ok(Some(row)) => {
                let real_id: String = row.get("message_id");
                real_id
            },
            Ok(None) => {
                tracing::error!("未找到要标记星标的邮件记录: {}", message_id);
                return Err("未找到要标记星标的邮件".to_string());
            },
            Err(e) => {
                tracing::error!("查询邮件记录失败: {}", e);
                return Err(format!("查询邮件记录失败: {}", e));
            }
        }
    };

    // 根据账户类型选择标记方式
    let is_gmail = account.email.ends_with("@gmail.com") ||
                   account.imap_server.contains("gmail");

    let is_outlook = (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) &&
                     account.auth_type.as_deref() == Some("oauth2");

    let is_qq = account.email.ends_with("@qq.com") ||
                account.imap_server.contains("qq.com");

    // 先尝试在服务器上标记星标状态
    if is_gmail && account.access_token.is_some() {
        // Gmail账户使用Gmail API标记
        match crate::gmail_api::GmailApiService::mark_as_starred(&account.access_token.unwrap_or_default(), &real_message_id, is_starred).await {
            Ok(_) => {
                tracing::info!("Gmail服务器已标记邮件为{}: {}", if is_starred { "星标" } else { "非星标" }, real_message_id);
            },
            Err(e) => {
                let error_str = e.to_string();
                tracing::debug!("Gmail API标记星标错误详情: {}", error_str);

                // 如果是401错误，尝试刷新令牌
                if error_str.contains("401") || error_str.contains("Unauthorized") || error_str.contains("InvalidAuthenticationToken") {
                    tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");

                    if let Some(refresh_token) = &account.refresh_token {
                        // 刷新Gmail令牌
                        let config = get_gmail_oauth2_config();
                        let client = OAuth2Client::new(config);
                        match client.refresh_token(refresh_token).await {
                            Ok(new_token) => {
                                tracing::info!("成功刷新Gmail访问令牌");

                                // 更新数据库中的令牌
                                if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                    tracing::error!("更新数据库中的令牌失败: {}", e);
                                }

                                // 使用新令牌重试标记星标
                                match crate::gmail_api::GmailApiService::mark_as_starred(&new_token.access_token, &real_message_id, is_starred).await {
                                    Ok(_) => {
                                        tracing::info!("使用新令牌成功标记邮件为{}: {}", if is_starred { "星标" } else { "非星标" }, real_message_id);
                                    },
                                    Err(retry_e) => {
                                        tracing::warn!("使用新令牌标记邮件星标状态仍然失败: {}", retry_e);
                                    }
                                }
                            },
                            Err(refresh_e) => {
                                tracing::error!("刷新Gmail令牌失败: {}", refresh_e);
                            }
                        }
                    } else {
                        tracing::error!("没有刷新令牌，无法刷新访问令牌");
                    }
                } else {
                    tracing::warn!("Gmail API标记邮件星标状态失败: {}", e);
                }
            }
        }
    } else if is_outlook && account.access_token.is_some() {
        // Outlook账户使用Graph API标记
        match crate::outlook_api::OutlookApiService::mark_as_starred(&account.access_token.unwrap_or_default(), &real_message_id, is_starred).await {
            Ok(_) => {
                tracing::info!("Outlook服务器已标记邮件为{}: {}", if is_starred { "星标" } else { "非星标" }, real_message_id);
            },
            Err(e) => {
                let error_str = e.to_string();
                tracing::debug!("Outlook Graph API标记星标错误详情: {}", error_str);

                // 如果是401错误，尝试刷新令牌
                if error_str.contains("401") || error_str.contains("Unauthorized") || error_str.contains("InvalidAuthenticationToken") {
                    tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");

                    if let Some(refresh_token) = &account.refresh_token {
                        // 刷新Outlook令牌
                        let config = get_outlook_oauth2_config();
                        let client = OAuth2Client::new(config);
                        match client.refresh_outlook_token(refresh_token).await {
                            Ok(new_token) => {
                                tracing::info!("成功刷新Outlook访问令牌");

                                // 更新数据库中的令牌
                                if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                    tracing::error!("更新数据库中的令牌失败: {}", e);
                                }

                                // 使用新令牌重试标记星标
                                match crate::outlook_api::OutlookApiService::mark_as_starred(&new_token.access_token, &real_message_id, is_starred).await {
                                    Ok(_) => {
                                        tracing::info!("使用新令牌成功标记邮件为{}: {}", if is_starred { "星标" } else { "非星标" }, real_message_id);
                                    },
                                    Err(retry_e) => {
                                        tracing::warn!("使用新令牌标记邮件星标状态仍然失败: {}", retry_e);
                                    }
                                }
                            },
                            Err(refresh_e) => {
                                tracing::error!("刷新Outlook令牌失败: {}", refresh_e);
                            }
                        }
                    } else {
                        tracing::error!("没有刷新令牌，无法刷新访问令牌");
                    }
                } else {
                    tracing::warn!("Outlook Graph API标记邮件星标状态失败: {}", e);
                }
            }
        }
    } else if is_qq {
        // QQ邮箱：使用IMAP FLAGS同步星标状态
        tracing::info!("使用IMAP FLAGS同步QQ邮箱星标状态");
        match crate::email_service::EmailService::mark_message_as_starred_imap(&account, &real_message_id, is_starred).await {
            Ok(success) => {
                if success {
                    tracing::info!("QQ邮箱IMAP服务器已标记邮件为{}: {}", if is_starred { "星标" } else { "非星标" }, real_message_id);
                } else {
                    tracing::warn!("QQ邮箱IMAP标记邮件星标状态失败：无法找到邮件，仅更新本地缓存");
                }
            },
            Err(e) => {
                tracing::warn!("QQ邮箱IMAP标记邮件星标状态失败: {}，仅更新本地缓存", e);
            }
        }
    } else {
        // 其他账户类型：暂不支持服务器端星标功能
        tracing::debug!("该账户类型暂不支持服务器端星标功能，仅更新本地缓存");
    }

    // 更新本地SQLite数据库中的邮件星标状态
    match email_database.update_email_status(&account_id, &real_message_id, None, Some(is_starred), None).await {
        Ok(updated) => {
            if updated {
                tracing::info!("本地缓存已标记邮件星标状态: {} -> {}", message_id, if is_starred { "已收藏" } else { "未收藏" });
                Ok(())
            } else {
                Err("邮件不存在或更新失败".to_string())
            }
        },
        Err(e) => {
            tracing::error!("更新本地邮件星标状态失败: {}", e);
            Err(format!("更新本地邮件星标状态失败: {}", e))
        }
    }
}

#[tauri::command]
async fn delete_message(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
    current_folder: Option<String>,
) -> Result<bool, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询账户失败: {}", e);
        format!("查询账户失败: {}", e)
    })?
    .ok_or("账户不存在或已禁用")?;

    let folder = current_folder.unwrap_or_else(|| "INBOX".to_string());

    // 映射文件夹名称
    let actual_folder = match state.folder_mapper.map_folder(&account, &folder).await {
        Ok(folder) => folder,
        Err(e) => {
            tracing::error!("文件夹映射失败: {}", e);
            return Err(format!("文件夹映射失败: {}", e));
        }
    };

    // 根据账户类型选择删除方式
    let is_gmail = account.email.ends_with("@gmail.com") ||
                   account.imap_server.contains("gmail");

    let is_outlook = (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) &&
                     account.auth_type.as_deref() == Some("oauth2");

    let delete_result = if is_gmail && account.access_token.is_some() {
        // Gmail账户使用Gmail API删除
        // 对于Gmail，我们需要使用邮件的内部ID而不是message_id
        // 首先从数据库获取邮件记录以获取Gmail的内部ID
        let email_database = {
            let guard = state.email_database.lock().unwrap();
            guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
        };

        // 通过message_id查找邮件记录，获取Gmail的内部ID（存储在id字段中）
        let gmail_internal_id = match sqlx::query("SELECT id FROM emails WHERE account_id = ? AND message_id = ?")
            .bind(&account_id)
            .bind(&message_id)
            .fetch_optional(email_database.pool())
            .await
        {
            Ok(Some(row)) => {
                let internal_id: String = row.get("id");
                internal_id
            },
            Ok(None) => {
                tracing::error!("未找到要删除的邮件记录: {}", message_id);
                return Err("未找到要删除的邮件".to_string());
            },
            Err(e) => {
                tracing::error!("查询邮件记录失败: {}", e);
                return Err(format!("查询邮件记录失败: {}", e));
            }
        };

        match crate::gmail_api::GmailApiService::delete_message(&account.access_token.unwrap_or_default(), &gmail_internal_id, &actual_folder).await {
            Ok(success) => Ok(success),
            Err(e) => Err(anyhow::anyhow!("Gmail API删除邮件失败: {}", e))
        }
    } else if is_outlook && account.access_token.is_some() {
        // Outlook账户使用Graph API删除
        match crate::outlook_api::OutlookApiService::delete_message(&account.access_token.unwrap_or_default(), &message_id, &actual_folder).await {
            Ok(success) => Ok(success),
            Err(e) => {
                let error_str = e.to_string();
                tracing::debug!("Outlook Graph API删除邮件错误详情: {}", error_str);

                // 如果是401错误，尝试刷新令牌
                if error_str.contains("401") || error_str.contains("Unauthorized") || error_str.contains("InvalidAuthenticationToken") {
                    tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");

                    if let Some(refresh_token) = &account.refresh_token {
                        // 刷新Outlook令牌
                        let config = get_outlook_oauth2_config();
                        let client = OAuth2Client::new(config);
                        match client.refresh_outlook_token(refresh_token).await {
                            Ok(new_token) => {
                                tracing::info!("成功刷新Outlook访问令牌");

                                // 更新数据库中的令牌
                                if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                    tracing::error!("更新数据库中的令牌失败: {}", e);
                                }

                                // 使用新令牌重试删除邮件
                                match crate::outlook_api::OutlookApiService::delete_message(&new_token.access_token, &message_id, &actual_folder).await {
                                    Ok(success) => {
                                        tracing::info!("使用新令牌成功删除邮件");
                                        Ok(success)
                                    },
                                    Err(retry_e) => {
                                        tracing::error!("使用新令牌删除邮件仍然失败: {}", retry_e);
                                        Err(anyhow::anyhow!("Outlook Graph API删除邮件失败: {}", retry_e))
                                    }
                                }
                            },
                            Err(refresh_error) => {
                                tracing::error!("刷新Outlook访问令牌失败: {}", refresh_error);
                                Err(anyhow::anyhow!("刷新Outlook访问令牌失败: {}", refresh_error))
                            }
                        }
                    } else {
                        tracing::error!("没有刷新令牌，无法刷新访问令牌");
                        Err(anyhow::anyhow!("没有刷新令牌，无法刷新访问令牌"))
                    }
                } else {
                    tracing::error!("Outlook Graph API删除邮件失败（非401错误）: {}", e);
                    Err(anyhow::anyhow!("Outlook Graph API删除邮件失败: {}", e))
                }
            }
        }
    } else {
        // 其他账户使用IMAP删除
        // 对于QQ邮箱等，先获取邮件元数据作为备用查找信息
        let email_database = {
            let guard = state.email_database.lock().unwrap();
            guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
        };

        // 直接删除，不暂停实时监听
        tracing::info!("开始删除邮件，保持实时监听运行");

        // 正确的删除流程：先删除服务器，再删除本地数据库
        let delete_result = EmailService::delete_message_simple(&account, &message_id, &actual_folder, &email_database).await;

        delete_result
    };

    match delete_result {
        Ok(success) => {
            if success {

                tracing::info!("邮件删除成功: {}", message_id);
                Ok(true)
            } else {
                Err("删除邮件失败".to_string())
            }
        },
        Err(e) => {
            tracing::error!("删除邮件失败: {}", e);
            Err(format!("删除邮件失败: {}", e))
        }
    }
}

#[tauri::command]
async fn move_message(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
    from_folder: String,
    to_folder: String,
) -> Result<bool, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询账户失败: {}", e);
        format!("查询账户失败: {}", e)
    })?
    .ok_or("账户不存在或已禁用")?;

    // 根据账户类型选择移动方式
    let is_gmail = account.email.ends_with("@gmail.com") ||
                   account.imap_server.contains("gmail");

    let is_outlook = (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) &&
                     account.auth_type.as_deref() == Some("oauth2");

    let move_result = if is_gmail && account.access_token.is_some() {
        // Gmail账户使用Gmail API移动
        match crate::gmail_api::GmailApiService::move_message(&account.access_token.unwrap_or_default(), &message_id, &from_folder, &to_folder).await {
            Ok(success) => Ok(success),
            Err(e) => Err(anyhow::anyhow!("Gmail API移动邮件失败: {}", e))
        }
    } else if is_outlook && account.access_token.is_some() {
        // Outlook账户使用Graph API移动
        match crate::outlook_api::OutlookApiService::move_message(&account.access_token.unwrap_or_default(), &message_id, &from_folder, &to_folder).await {
            Ok(success) => Ok(success),
            Err(e) => {
                let error_str = e.to_string();
                tracing::debug!("Outlook Graph API移动邮件错误详情: {}", error_str);

                // 如果是401错误，尝试刷新令牌
                if error_str.contains("401") || error_str.contains("Unauthorized") || error_str.contains("InvalidAuthenticationToken") {
                    tracing::info!("检测到令牌过期，尝试刷新OAuth2访问令牌");

                    if let Some(refresh_token) = &account.refresh_token {
                        // 刷新Outlook令牌
                        let config = get_outlook_oauth2_config();
                        let client = OAuth2Client::new(config);
                        match client.refresh_outlook_token(refresh_token).await {
                            Ok(new_token) => {
                                tracing::info!("成功刷新Outlook访问令牌");

                                // 更新数据库中的令牌
                                if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                    tracing::error!("更新数据库中的令牌失败: {}", e);
                                }

                                // 使用新令牌重试移动邮件
                                match crate::outlook_api::OutlookApiService::move_message(&new_token.access_token, &message_id, &from_folder, &to_folder).await {
                                    Ok(success) => {
                                        tracing::info!("使用新令牌成功移动邮件");
                                        Ok(success)
                                    },
                                    Err(retry_e) => {
                                        tracing::error!("使用新令牌移动邮件仍然失败: {}", retry_e);
                                        Err(anyhow::anyhow!("Outlook Graph API移动邮件失败: {}", retry_e))
                                    }
                                }
                            },
                            Err(refresh_e) => {
                                tracing::error!("刷新Outlook令牌失败: {}", refresh_e);
                                Err(anyhow::anyhow!("Outlook Graph API移动邮件失败，令牌刷新也失败: {}", e))
                            }
                        }
                    } else {
                        tracing::error!("没有刷新令牌，无法刷新访问令牌");
                        Err(anyhow::anyhow!("Outlook Graph API移动邮件失败: {}", e))
                    }
                } else {
                    Err(anyhow::anyhow!("Outlook Graph API移动邮件失败: {}", e))
                }
            }
        }
    } else {
        // 其他账户暂不支持移动功能
        Err(anyhow::anyhow!("该账户类型暂不支持移动邮件功能"))
    };

    match move_result {
        Ok(success) => {
            if success {
                // 移动成功后，更新本地SQLite缓存
                let email_database = {
                    let guard = state.email_database.lock().unwrap();
                    guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
                };

                // 从本地缓存中删除邮件记录（因为它已经移动到其他文件夹）
                match email_database.delete_email(&account_id, &message_id).await {
                    Ok(deleted) => {
                        if deleted {
                            tracing::info!("已从本地缓存删除移动的邮件: {}", message_id);
                        } else {
                            tracing::warn!("本地缓存中未找到要删除的邮件: {}", message_id);
                        }
                    },
                    Err(e) => {
                        tracing::warn!("删除本地缓存邮件失败: {}", e);
                        // 不阻止移动操作的成功返回
                    }
                }

                tracing::info!("邮件移动成功: {} 从 {} 到 {}", message_id, from_folder, to_folder);
                Ok(true)
            } else {
                Err("移动邮件失败".to_string())
            }
        },
        Err(e) => {
            tracing::error!("移动邮件失败: {}", e);
            Err(format!("移动邮件失败: {}", e))
        }
    }
}

#[tauri::command]
async fn refresh_messages(
    state: State<'_, AppState>,
    account_id: String,
    folder: Option<String>,
) -> Result<Vec<EmailMessage>, String> {
    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
    };

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| format!("查询账户失败: {}", e))?
    .ok_or("账户不存在或已禁用")?;

    let folder_name = folder.unwrap_or_else(|| "INBOX".to_string());

    tracing::info!("刷新特定文件夹: {} - {}", account_id, folder_name);

    // 只删除特定文件夹的缓存
    if let Err(e) = email_database.delete_emails_by_folder(&account_id, &folder_name).await {
        tracing::warn!("删除文件夹缓存失败: {}", e);
    }

    // 从服务器重新获取该文件夹的邮件
    let messages = match account.auth_type.as_deref() {
        Some("oauth2") => {
            if account.imap_server.contains("outlook") || account.imap_server.contains("graph.microsoft.com") {
                // Outlook账户
                let access_token = account.access_token.as_ref()
                    .ok_or("Outlook账户缺少访问令牌")?;
                outlook_api::OutlookApiService::fetch_messages(access_token, &folder_name, None).await
                    .map_err(|e| format!("获取Outlook邮件失败: {}", e))?
            } else {
                // Gmail账户
                let access_token = account.access_token.as_ref()
                    .ok_or("Gmail账户缺少访问令牌")?;
                gmail_api::GmailApiService::fetch_messages(access_token, &folder_name, None).await
                    .map_err(|e| format!("获取Gmail邮件失败: {}", e))?
            }
        },
        _ => {
            // IMAP账户
            email_service::EmailService::fetch_messages(&account, &folder_name, None).await
                .map_err(|e| format!("获取IMAP邮件失败: {}", e))?
        }
    };

    // 保存到SQLite数据库
    if let Err(e) = email_database.save_emails(&messages).await {
        tracing::warn!("保存邮件到SQLite失败: {}", e);
    }

    tracing::info!("成功刷新文件夹 {} 的 {} 封邮件", folder_name, messages.len());
    Ok(messages)
}

#[tauri::command]
async fn clear_cache(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> Result<(), String> {
    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    if let Some(account_id) = account_id {
        match email_database.delete_emails_by_account(&account_id).await {
            Ok(deleted_count) => {
                tracing::info!("已清除账户 {} 的缓存，删除了 {} 封邮件", account_id, deleted_count);
                Ok(())
            },
            Err(e) => {
                tracing::error!("清除账户缓存失败: {}", e);
                Err(format!("清除账户缓存失败: {}", e))
            }
        }
    } else {
        match email_database.clear_cache().await {
            Ok(deleted_count) => {
                tracing::info!("清空SQLite邮件缓存，删除了 {} 条记录", deleted_count);

                // 重要：清除缓存后，停止实时监听，清除初始化状态，强制所有账户重新进行初始加载
                {
                    let mut sync_enabled = state.auto_sync_enabled.lock().unwrap();
                    *sync_enabled = false;
                    tracing::info!("已停止实时监听，等待重新初始化");
                }

                // 清除所有账户的初始化状态和首次加载状态
                {
                    let mut initialized = state.initialized_accounts.lock().unwrap();
                    initialized.clear();
                    tracing::info!("已清除所有账户的初始化状态");
                }
                // 清除初始化状态已在上面完成

                // 重要：清空缓存后，确保实时监听完全停止，直到所有账户重新完成首次加载
                *state.auto_sync_enabled.lock().unwrap() = false;
                tracing::info!("清空缓存后停止实时监听，等待所有账户重新完成首次加载");

                tracing::info!("成功清空SQLite邮件缓存，删除了 {} 条记录", deleted_count);
                Ok(())
            },
            Err(e) => {
                tracing::error!("清除所有缓存失败: {}", e);
                Err(format!("清除所有缓存失败: {}", e))
            }
        }
    }
}

#[tauri::command]
async fn get_cache_stats(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, i64>, String> {
    let _email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 这里可以实现获取SQLite数据库统计信息
    // 暂时返回空的统计信息
    let mut stats = std::collections::HashMap::new();
    stats.insert("total_emails".to_string(), 0);
    Ok(stats)
}



#[tauri::command]
async fn get_account_folders(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<std::collections::HashMap<String, String>, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询账户失败: {}", e);
        format!("查询账户失败: {}", e)
    })?
    .ok_or("账户不存在或已禁用")?;

    // 获取文件夹映射
    match state.folder_mapper.get_folder_mappings(&account).await {
        Ok(mappings) => {
            tracing::info!("成功获取账户 {} 的文件夹映射", account.name);
            Ok(mappings)
        },
        Err(e) => {
            tracing::error!("获取文件夹映射失败: {}", e);
            Err(format!("获取文件夹映射失败: {}", e))
        }
    }
}

#[tauri::command]
async fn analyze_email_tags(
    state: State<'_, AppState>,
    subject: String,
    body: String,
) -> Result<Vec<EmailTag>, String> {
    let analysis = state.email_analyzer.analyze_email(&subject, &body);
    let tags = state.email_analyzer.get_display_tags(&analysis);
    Ok(tags)
}

// 立即发送桌面通知的命令
#[tauri::command]
async fn send_desktop_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;

    // 立即发送通知
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .icon("icon.png")
        .show()
        .map_err(|e| format!("发送通知失败: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn get_starred_messages(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<EmailMessage>, String> {
    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 直接从数据库查询收藏邮件
    match email_database.get_starred_emails(&account_id).await {
        Ok(starred_emails) => {
            tracing::info!("获取到 {} 封收藏邮件", starred_emails.len());
            Ok(starred_emails)
        },
        Err(e) => {
            tracing::error!("获取收藏邮件失败: {}", e);
            Err(format!("获取收藏邮件失败: {}", e))
        }
    }
}

// 启动自动同步
#[tauri::command]
async fn start_auto_sync(
    state: State<'_, AppState>,
    _interval_minutes: Option<u64>, // 保留参数兼容性，但不再使用
) -> Result<String, String> {
    // 更新同步设置
    *state.auto_sync_enabled.lock().unwrap() = true;

    tracing::info!("启动自动邮件同步（每3秒同步一次）");
    Ok("自动同步已启动（每3秒同步一次）".to_string())
}

// 停止自动同步
#[tauri::command]
async fn stop_auto_sync(state: State<'_, AppState>) -> Result<String, String> {
    *state.auto_sync_enabled.lock().unwrap() = false;

    tracing::info!("停止自动邮件同步");
    Ok("自动同步已停止".to_string())
}

// 获取自动同步状态
#[tauri::command]
async fn get_auto_sync_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let enabled = *state.auto_sync_enabled.lock().unwrap();

    Ok(serde_json::json!({
        "enabled": enabled,
        "interval_seconds": 3  // 固定3秒间隔
    }))
}

// 通知初始邮件加载完成，可以启动实时监听
#[tauri::command]
async fn notify_initial_load_complete(state: State<'_, AppState>) -> Result<String, String> {
    // 检查当前是否处于暂停状态（如清空缓存期间）
    let current_state = *state.auto_sync_enabled.lock().unwrap();

    // 只有在当前未暂停的情况下才启动实时监听
    if current_state {
        tracing::debug!("实时监听已启动，跳过重复启动");
        Ok("实时邮件监听已启动".to_string())
    } else {
        // 检查是否有正在进行的操作（通过检查是否有暂停标记）
        // 这里我们需要一个更智能的方式来判断是否应该启动
        // 暂时延迟启动，让清空缓存等操作有时间完成
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 再次检查状态
        let final_state = *state.auto_sync_enabled.lock().unwrap();
        if !final_state {
            *state.auto_sync_enabled.lock().unwrap() = true;
            tracing::info!("初始邮件加载完成，启动实时邮件监听");
            Ok("实时邮件监听已启动".to_string())
        } else {
            tracing::debug!("实时监听已在其他地方启动");
            Ok("实时邮件监听已启动".to_string())
        }
    }
}

// 检查新邮件（用于自动同步）
#[tauri::command]
async fn check_new_messages(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<usize, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
    };

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 验证账户ID格式
    uuid::Uuid::parse_str(&account_id)
        .map_err(|e| format!("无效的账户ID: {}", e))?;

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(&account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询账户失败: {}", e);
        format!("查询账户失败: {}", e)
    })?
    .ok_or("账户不存在或已禁用")?;

    let folder = "INBOX";

    // 映射文件夹名称
    let actual_folder = match state.folder_mapper.map_folder(&account, folder).await {
        Ok(folder) => folder,
        Err(e) => {
            tracing::error!("文件夹映射失败: {}", e);
            return Err(format!("文件夹映射失败: {}", e));
        }
    };

    // 从服务器获取最新邮件（获取所有邮件以确保不遗漏）
    // 检查账户类型，使用对应的API
    let messages = if account.imap_server.contains("gmail") && account.auth_type.as_deref() == Some("oauth2") {
        tracing::debug!("实时监听：使用Gmail API检查新邮件");
        if let Some(access_token) = &account.access_token {
            match gmail_api::GmailApiService::fetch_messages(access_token, &actual_folder, Some(5)).await {
                Ok(mut messages) => {
                    // 设置account_id
                    for message in &mut messages {
                        message.account_id = account_id.clone();
                    }
                    messages
                },
                Err(e) => {
                    tracing::debug!("实时监听：Gmail API获取邮件失败: {}", e);
                    return Err(format!("Gmail API获取邮件失败: {}", e));
                }
            }
        } else {
            return Err("Gmail账户缺少访问令牌".to_string());
        }
    } else if (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) && account.auth_type.as_deref() == Some("oauth2") {
        // 使用Outlook Graph API
        tracing::debug!("实时监听：使用Outlook Graph API检查新邮件");
        if let Some(access_token) = &account.access_token {
            match outlook_api::OutlookApiService::fetch_messages(access_token, &actual_folder, Some(5)).await {
                Ok(mut messages) => {
                    // 设置account_id
                    for message in &mut messages {
                        message.account_id = account_id.clone();
                    }
                    messages
                },
                Err(e) => {
                    tracing::debug!("实时监听：Outlook Graph API获取邮件失败: {}", e);
                    return Err(format!("Outlook Graph API获取邮件失败: {}", e));
                }
            }
        } else {
            return Err("Outlook账户缺少访问令牌".to_string());
        }
    } else {
        // 使用传统IMAP方式
        match EmailService::fetch_messages_with_db(&account, &actual_folder, None, Some(&database)).await {
            Ok(messages) => messages,
            Err(e) => {
                tracing::error!("实时监听：IMAP获取邮件失败: {}", e);
                return Err(format!("IMAP获取邮件失败: {}", e));
            }
        }
    };

    // 保存到SQLite数据库，save_emails现在返回新增邮件的数量
    match email_database.save_emails(&messages).await {
        Ok(new_emails_count) => {
            if new_emails_count > 0 {
                tracing::info!("检查到 {} 封新邮件", new_emails_count);
            }
            Ok(new_emails_count)
        },
        Err(e) => {
            tracing::warn!("保存邮件到SQLite失败: {}", e);
            Ok(0) // 保存失败时返回0
        }
    }
}

#[tauri::command]
async fn clear_email_cache(state: State<'_, AppState>) -> Result<String, String> {
    // 暂时停止实时监听，避免与清空缓存操作冲突
    let was_enabled = *state.auto_sync_enabled.lock().unwrap();
    *state.auto_sync_enabled.lock().unwrap() = false;
    tracing::info!("清空缓存期间暂停实时监听");

    // 等待足够长的时间确保实时监听任务完全停止
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    let result = match email_database.clear_cache().await {
        Ok(count) => {
            tracing::info!("成功清空SQLite邮件缓存，删除了 {} 条记录", count);

            // 清空缓存后，自动重新初始化所有账户
            tracing::info!("清空缓存后，开始自动重新初始化所有账户...");

            let database = {
                let guard = state.database.lock().unwrap();
                guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
            };

            // 获取所有活跃账户
            let accounts_result = sqlx::query_as::<_, EmailAccount>(
                "SELECT * FROM email_accounts WHERE is_active = TRUE"
            )
            .fetch_all(&database)
            .await;

            match accounts_result {
                Ok(accounts) => {
                    if !accounts.is_empty() {
                        let accounts_count = accounts.len();
                        tracing::info!("找到 {} 个活跃账户，开始重新初始化", accounts_count);
                        let mut success_count = 0;

                        for account in accounts {
                            tracing::info!("正在重新初始化账户: {}", account.name);

                            // 为每个账户重新加载邮件（优先使用缓存，缓存为空时才从服务器获取）
                            match get_messages(state.clone(), account.id.clone(), Some("INBOX".to_string()), Some(200), Some(false)).await {
                                Ok(_) => {
                                    success_count += 1;
                                    tracing::info!("账户 {} 重新初始化成功", account.name);
                                },
                                Err(e) => {
                                    tracing::warn!("账户 {} 重新初始化失败: {}", account.name, e);
                                }
                            }
                        }

                        tracing::info!("账户重新初始化完成，成功 {}/{} 个账户", success_count, accounts_count);
                        Ok(format!("成功清空邮件缓存，删除了 {} 条记录，并重新初始化了 {}/{} 个账户", count, success_count, accounts_count))
                    } else {
                        Ok(format!("成功清空邮件缓存，删除了 {} 条记录", count))
                    }
                },
                Err(e) => {
                    tracing::error!("获取账户列表失败: {}", e);
                    Ok(format!("成功清空邮件缓存，删除了 {} 条记录，但重新初始化账户失败: {}", count, e))
                }
            }
        },
        Err(e) => {
            tracing::error!("清空邮件缓存失败: {}", e);
            Err(format!("清空邮件缓存失败: {}", e))
        }
    };

    // 等待适当时间确保清空缓存和重新获取邮件操作完成，然后恢复实时监听
    // 优化为5秒，平衡用户体验和功能稳定性
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    *state.auto_sync_enabled.lock().unwrap() = was_enabled;

    result
}

#[tauri::command]
async fn debug_email_status(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<serde_json::Value, String> {
    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 获取邮件统计信息
    let stats = sqlx::query(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(CASE WHEN is_read = 1 THEN 1 END) as read_count,
            COUNT(CASE WHEN is_read = 0 THEN 1 END) as unread_count,
            COUNT(CASE WHEN is_deleted = 1 THEN 1 END) as deleted_count
        FROM emails
        WHERE account_id = ?
        "#
    )
    .bind(&account_id)
    .fetch_one(email_database.pool())
    .await
    .map_err(|e| format!("查询邮件统计失败: {}", e))?;

    let total: i64 = stats.get("total");
    let read_count: i64 = stats.get("read_count");
    let unread_count: i64 = stats.get("unread_count");
    let deleted_count: i64 = stats.get("deleted_count");

    // 获取前10封邮件的详细状态
    let sample_emails = sqlx::query(
        "SELECT id, subject, is_read, is_deleted FROM emails WHERE account_id = ? ORDER BY received_at DESC LIMIT 10"
    )
    .bind(&account_id)
    .fetch_all(email_database.pool())
    .await
    .map_err(|e| format!("查询邮件样本失败: {}", e))?;

    let mut samples = Vec::new();
    for row in sample_emails {
        samples.push(serde_json::json!({
            "id": row.get::<String, _>("id"),
            "subject": row.get::<String, _>("subject"),
            "is_read": row.get::<bool, _>("is_read"),
            "is_deleted": row.get::<bool, _>("is_deleted")
        }));
    }

    tracing::info!("邮件状态调试 - 总计:{}, 已读:{}, 未读:{}, 已删除:{}", total, read_count, unread_count, deleted_count);

    Ok(serde_json::json!({
        "total": total,
        "read_count": read_count,
        "unread_count": unread_count,
        "deleted_count": deleted_count,
        "sample_emails": samples
    }))
}



#[tauri::command]
fn get_email_providers() -> Vec<EmailProviderConfig> {
    vec![
        EmailProviderConfig {
            name: "Gmail".to_string(),
            imap_server: "imap.gmail.com".to_string(),
            imap_port: 993,
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 587,
            use_tls: true,
        },
        EmailProviderConfig {
            name: "Outlook".to_string(),
            imap_server: "outlook.office365.com".to_string(),
            imap_port: 993,
            smtp_server: "smtp-mail.outlook.com".to_string(),
            smtp_port: 587,
            use_tls: true,
        },
        EmailProviderConfig {
            name: "QQ邮箱".to_string(),
            imap_server: "imap.qq.com".to_string(),
            imap_port: 993,
            smtp_server: "smtp.qq.com".to_string(),
            smtp_port: 465,
            use_tls: true,
        },
        EmailProviderConfig {
            name: "163邮箱".to_string(),
            imap_server: "imap.163.com".to_string(),
            imap_port: 993,
            smtp_server: "smtp.163.com".to_string(),
            smtp_port: 587,
            use_tls: true,
        },
        EmailProviderConfig {
            name: "126邮箱".to_string(),
            imap_server: "imap.126.com".to_string(),
            imap_port: 993,
            smtp_server: "smtp.126.com".to_string(),
            smtp_port: 587,
            use_tls: true,
        },
        EmailProviderConfig {
            name: "Yahoo邮箱".to_string(),
            imap_server: "imap.mail.yahoo.com".to_string(),
            imap_port: 993,
            smtp_server: "smtp.mail.yahoo.com".to_string(),
            smtp_port: 587,
            use_tls: true,
        },
        EmailProviderConfig {
            name: "iCloud".to_string(),
            imap_server: "imap.mail.me.com".to_string(),
            imap_port: 993,
            smtp_server: "smtp.mail.me.com".to_string(),
            smtp_port: 587,
            use_tls: true,
        },
    ]
}

// 用户注册命令
#[tauri::command]
async fn register_user(
    state: State<'_, AppState>,
    user_data: NewUser
) -> Result<String, String> {
    // 确保数据库已初始化
    let database = {
        // 检查数据库是否已初始化
        let needs_init = {
            let guard = state.database.lock().unwrap();
            guard.is_none()
        };

        if needs_init {
            tracing::info!("数据库未初始化，开始初始化数据库...");
            // 初始化数据库（在锁外进行异步操作）
            match Database::new().await {
                Ok(db) => {
                    tracing::info!("数据库初始化成功");
                    // 重新获取锁并设置数据库
                    let mut guard = state.database.lock().unwrap();
                    *guard = Some(db);
                }
                Err(e) => {
                    tracing::error!("数据库初始化失败: {}", e);
                    return Err(format!("数据库初始化失败: {}", e));
                }
            }
        }

        // 获取数据库连接池
        let guard = state.database.lock().unwrap();
        guard.as_ref().unwrap().pool().clone()
    };

    // 验证输入
    if user_data.username.trim().is_empty() {
        return Err("用户名不能为空".to_string());
    }
    if user_data.email.trim().is_empty() {
        return Err("邮箱不能为空".to_string());
    }
    if user_data.password.len() < 6 {
        return Err("密码长度至少6位".to_string());
    }

    // 检查用户名是否已存在
    let existing_user = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE username = ? OR email = ?"
    )
    .bind(&user_data.username)
    .bind(&user_data.email)
    .fetch_one(&database)
    .await
    .map_err(|e| {
        tracing::error!("检查用户是否存在失败: {}", e);
        format!("检查用户是否存在失败: {}", e)
    })?;

    if existing_user > 0 {
        return Err("用户名或邮箱已存在".to_string());
    }

    // 加密密码
    let password_hash = bcrypt::hash(&user_data.password, bcrypt::DEFAULT_COST)
        .map_err(|e| {
            tracing::error!("密码加密失败: {}", e);
            format!("密码加密失败: {}", e)
        })?;

    let id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();

    // 插入用户数据
    sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, created_at, updated_at, is_active)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(id.to_string())
    .bind(&user_data.username)
    .bind(&user_data.email)
    .bind(&password_hash)
    .bind(now)
    .bind(now)
    .bind(true)
    .execute(&database)
    .await
    .map_err(|e| {
        tracing::error!("创建用户失败: {}", e);
        format!("创建用户失败: {}", e)
    })?;

    tracing::info!("用户注册成功: {}", user_data.username);
    Ok("注册成功".to_string())
}

// OAuth2用户登录命令（自动注册）
#[tauri::command]
async fn oauth2_login_user(
    state: State<'_, AppState>,
    user_info: serde_json::Value
) -> Result<LoginResponse, String> {
    tracing::info!("OAuth2用户登录开始，用户信息: {}", user_info);

    // 确保数据库已初始化
    let database = {
        // 检查数据库是否已初始化
        let needs_init = {
            let guard = state.database.lock().unwrap();
            guard.is_none()
        };

        if needs_init {
            tracing::info!("数据库未初始化，开始初始化数据库...");
            // 初始化数据库（在锁外进行异步操作）
            match Database::new().await {
                Ok(db) => {
                    tracing::info!("数据库初始化成功");
                    // 重新获取锁并设置数据库
                    let mut guard = state.database.lock().unwrap();
                    *guard = Some(db);
                }
                Err(e) => {
                    tracing::error!("数据库初始化失败: {}", e);
                    return Err(format!("数据库初始化失败: {}", e));
                }
            }
        }

        // 获取数据库连接池
        let guard = state.database.lock().unwrap();
        guard.as_ref().unwrap().pool().clone()
    };

    // 从OAuth2用户信息中提取数据
    let email = user_info.get("email")
        .and_then(|v| v.as_str())
        .ok_or("OAuth2用户信息中缺少邮箱")?;

    // 支持QQ登录的用户名提取
    let username = user_info.get("nickname")
        .and_then(|v| v.as_str())
        .or_else(|| user_info.get("name").and_then(|v| v.as_str()))
        .unwrap_or(email); // 如果没有nickname或name，使用email作为用户名

    let auth_type = user_info.get("auth_type")
        .and_then(|v| v.as_str())
        .unwrap_or("oauth2");

    let provider = user_info.get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // 提取头像信息（支持多种字段名）
    let avatar = user_info.get("avatar")
        .and_then(|v| v.as_str())
        .or_else(|| user_info.get("picture").and_then(|v| v.as_str())) // Google API使用picture字段
        .filter(|s| !s.is_empty()); // 过滤空字符串

    tracing::info!("提取的用户信息 - 邮箱: {}, 用户名: {}, 认证类型: {}, 提供商: {}, 头像: {:?}", email, username, auth_type, provider, avatar);

    // 检查用户是否已存在（通过邮箱或用户名）
    let existing_user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = ? OR username = ?"
    )
    .bind(email)
    .bind(username)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询OAuth2用户失败: {}", e);
        format!("查询OAuth2用户失败: {}", e)
    })?;

    let user = if let Some(mut user) = existing_user {
        // 用户已存在，检查是否需要更新头像
        tracing::info!("OAuth2用户已存在: {} (用户名: {})", user.email, user.username);

        // 如果提供了新的头像URL且与现有头像不同，则更新
        if let Some(new_avatar) = avatar {
            if user.avatar.as_ref() != Some(&new_avatar.to_string()) {
                tracing::info!("更新用户头像: {} -> {}", user.avatar.as_deref().unwrap_or("无"), new_avatar);

                // 更新数据库中的头像
                sqlx::query("UPDATE users SET avatar = ?, updated_at = ? WHERE id = ?")
                    .bind(new_avatar)
                    .bind(chrono::Utc::now())
                    .bind(&user.id)
                    .execute(&database)
                    .await
                    .map_err(|e| {
                        tracing::error!("更新用户头像失败: {}", e);
                        format!("更新用户头像失败: {}", e)
                    })?;

                // 更新本地用户对象
                user.avatar = Some(new_avatar.to_string());
                user.updated_at = chrono::Utc::now();
            }
        }

        user
    } else {
        // 用户不存在，自动创建
        tracing::info!("创建新的OAuth2用户: {}", email);
        tracing::info!("开始在数据库中创建用户...");

        let id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        // 为OAuth2用户生成一个随机密码（不会被使用）
        let dummy_password = uuid::Uuid::new_v4().to_string();
        let password_hash = bcrypt::hash(&dummy_password, bcrypt::DEFAULT_COST)
            .map_err(|e| {
                tracing::error!("密码加密失败: {}", e);
                format!("密码加密失败: {}", e)
            })?;

        // 插入新用户
        sqlx::query(
            r#"
            INSERT INTO users (id, username, email, password_hash, created_at, updated_at, is_active, avatar)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(id.to_string())
        .bind(username)
        .bind(email)
        .bind(&password_hash)
        .bind(now)
        .bind(now)
        .bind(true)
        .bind(avatar)
        .execute(&database)
        .await
        .map_err(|e| {
            tracing::error!("创建OAuth2用户失败: {}", e);
            format!("创建OAuth2用户失败: {}", e)
        })?;

        tracing::info!("OAuth2用户创建成功，ID: {}", id);

        // 返回新创建的用户
        User {
            id: id.to_string(),
            username: username.to_string(),
            email: email.to_string(),
            password_hash,
            created_at: now,
            updated_at: now,
            is_active: true,
            avatar: avatar.map(|s| s.to_string()),
        }
    };



    tracing::info!("OAuth2用户登录成功: {}", user.email);
    Ok(LoginResponse {
        success: true,
        message: "OAuth2登录成功".to_string(),
        user: Some(UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            avatar: user.avatar,
        }),
    })
}

// 用户登录命令
#[tauri::command]
async fn login_user(
    state: State<'_, AppState>,
    login_data: LoginRequest
) -> Result<LoginResponse, String> {
    // 确保数据库已初始化
    let database = {
        // 检查数据库是否已初始化
        let needs_init = {
            let guard = state.database.lock().unwrap();
            guard.is_none()
        };

        if needs_init {
            tracing::info!("数据库未初始化，开始初始化数据库...");
            // 初始化数据库（在锁外进行异步操作）
            match Database::new().await {
                Ok(db) => {
                    tracing::info!("数据库初始化成功");
                    // 重新获取锁并设置数据库
                    let mut guard = state.database.lock().unwrap();
                    *guard = Some(db);
                }
                Err(e) => {
                    tracing::error!("数据库初始化失败: {}", e);
                    return Err(format!("数据库初始化失败: {}", e));
                }
            }
        }

        // 获取数据库连接池
        let guard = state.database.lock().unwrap();
        guard.as_ref().unwrap().pool().clone()
    };

    // 验证输入
    if login_data.username.trim().is_empty() {
        return Ok(LoginResponse {
            success: false,
            message: "用户名不能为空".to_string(),
            user: None,
        });
    }
    if login_data.password.trim().is_empty() {
        return Ok(LoginResponse {
            success: false,
            message: "密码不能为空".to_string(),
            user: None,
        });
    }

    // 查找用户
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE username = ? AND is_active = TRUE"
    )
    .bind(&login_data.username)
    .fetch_optional(&database)
    .await
    .map_err(|e| {
        tracing::error!("查询用户失败: {}", e);
        format!("查询用户失败: {}", e)
    })?;

    let user = match user {
        Some(user) => user,
        None => {
            return Ok(LoginResponse {
                success: false,
                message: "用户名或密码错误".to_string(),
                user: None,
            });
        }
    };

    // 验证密码
    let password_valid = bcrypt::verify(&login_data.password, &user.password_hash)
        .map_err(|e| {
            tracing::error!("密码验证失败: {}", e);
            format!("密码验证失败: {}", e)
        })?;

    if !password_valid {
        return Ok(LoginResponse {
            success: false,
            message: "用户名或密码错误".to_string(),
            user: None,
        });
    }

    tracing::info!("用户登录成功: {}", user.username);
    Ok(LoginResponse {
        success: true,
        message: "登录成功".to_string(),
        user: Some(UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            avatar: user.avatar,
        }),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 加载.env文件
    if let Err(e) = dotenv::dotenv() {
        tracing::warn!("无法加载.env文件: {}", e);
    } else {
        tracing::info!(".env文件加载成功");
    }

    // 设置环境变量来完全屏蔽Tao事件循环警告
    std::env::set_var("RUST_LOG", "warn,tauri_app_lib=info,tao::platform_impl::platform::event_loop::runner=off");

    let app_state = AppState {
        database: Mutex::new(None),
        email_database: Mutex::new(None),
        folder_mapper: FolderMapper::new(),
        email_analyzer: EmailAnalyzer::new(),
        auto_sync_enabled: Mutex::new(false), // 实时监听开关
        sync_interval_minutes: Mutex::new(1), // 保留字段兼容性
        initialized_accounts: Mutex::new(std::collections::HashSet::new()), // 初始化账户跟踪
    };

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            tracing::info!("Tauri应用setup函数开始执行");
            let app_handle = app.handle().clone();

            // 获取主窗口并设置事件处理
            if let Some(window) = app.get_webview_window("main") {
                // 设置窗口事件处理器来减少事件循环警告
                let _ = window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Resized(_) => {
                            // 处理窗口大小调整事件
                        },
                        tauri::WindowEvent::Moved(_) => {
                            // 处理窗口移动事件
                        },
                        _ => {}
                    }
                });
            }

            // 启动实时邮件监听任务
            tracing::info!("准备启动实时邮件监听任务");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                tracing::info!("实时邮件监听任务开始运行");
                real_time_email_listener(app_handle_clone).await;
            });

            // 启动定期令牌刷新任务
            tracing::info!("准备启动定期令牌刷新任务");
            let app_handle_clone2 = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                tracing::info!("定期令牌刷新任务开始运行");
                periodic_token_refresh(app_handle_clone2).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            init_database,
            create_email_account,
            get_email_accounts,
            delete_email_account,
            test_account_connection,
            test_email_connection,
            get_messages,
            refresh_messages,
            clear_cache,
            get_cache_stats,
            get_account_folders,
            analyze_email_tags,
            get_starred_messages,
            send_email,
            save_draft,
            mark_message_as_read,
            mark_message_as_starred,
            delete_message,
            move_message,
            get_email_providers,
            start_auto_sync,
            stop_auto_sync,
            get_auto_sync_status,
            notify_initial_load_complete,
            check_new_messages,
            clear_email_cache,
            debug_email_status,
            send_desktop_notification,
            // OAuth2 相关命令
            get_gmail_oauth_url,
            exchange_oauth_code,
            refresh_oauth_token,
            verify_oauth_token,
            refresh_account_token,
            complete_gmail_oauth2,
            start_oauth2_window,
            start_outlook_oauth2_window,
            // 用户认证相关命令
            register_user,
            login_user,
            oauth2_login_user,
            // QQ OAuth2 相关命令（简化模式）
            qq_oauth2_simple::start_qq_oauth2_login,
            qq_oauth2_simple::handle_qq_callback_legacy,
            qq_oauth2_simple::handle_qq_protocol_callback,
            // 数据库路径管理命令
            get_database_path,
            select_database_path,
            set_database_path,
            reset_database_path,
            // 联系人管理命令
            create_contact,
            get_contacts,
            update_contact,
            delete_contact
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// 实时邮件监听任务
async fn real_time_email_listener(app_handle: tauri::AppHandle) {
    tracing::info!("实时邮件监听任务启动，等待初始加载完成信号...");

    // 等待初始加载完成信号
    loop {
        time::sleep(Duration::from_secs(5)).await;

        let state = app_handle.state::<AppState>();
        let sync_enabled = *state.auto_sync_enabled.lock().unwrap();

        if sync_enabled {
            tracing::info!("收到初始加载完成信号，开始实时邮件监听");
            break;
        }
    }

    let mut interval = time::interval(Duration::from_secs(3)); // 每3秒检查一次新邮件
    let mut consecutive_errors = 0; // 连续错误计数
    let max_consecutive_errors = 5; // 最大连续错误次数
    let mut last_health_check = std::time::Instant::now(); // 上次健康检查时间
    let mut last_connection_reset = std::time::Instant::now(); // 上次连接重置时间
    let connection_reset_interval = std::time::Duration::from_secs(1800); // 30分钟重置一次连接
    let mut network_down = false; // 网络状态标记
    let mut last_network_check = std::time::Instant::now(); // 上次网络检查时间

    loop {
        interval.tick().await;

        // 获取应用状态
        let state = app_handle.state::<AppState>();

        // 检查实时监听是否被暂停
        if !*state.auto_sync_enabled.lock().unwrap() {
            tracing::debug!("实时监听已暂停，等待15秒后再次检查");
            // 暂停期间等待更长时间，避免频繁检查，确保清空缓存操作完成
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
            continue;
        }

        // 定期健康检查（每5分钟）
        if last_health_check.elapsed() > std::time::Duration::from_secs(300) {
            let network_status = if network_down { "🔴 网络断开" } else { "🟢 网络正常" };
            tracing::info!("实时监听健康检查：监听正常运行，连续错误次数: {}，网络状态: {}", consecutive_errors, network_status);
            last_health_check = std::time::Instant::now();
        }

        // 定期重置连接（每30分钟），特别针对QQ邮箱等可能有连接时间限制的服务器
        if last_connection_reset.elapsed() > connection_reset_interval {
            tracing::info!("定期连接重置：为保证连接稳定性，重置所有邮箱连接");
            last_connection_reset = std::time::Instant::now();
            // 这里可以添加清理连接池的逻辑
        }

        tracing::debug!("实时监听：开始检查新邮件...");

        // 获取所有活跃账户
        let database = {
            let guard = state.database.lock().unwrap();
            if let Some(db) = guard.as_ref() {
                db.pool().clone()
            } else {
                continue;
            }
        };

        let accounts_result = sqlx::query_as::<_, EmailAccount>(
            "SELECT * FROM email_accounts WHERE is_active = TRUE"
        )
        .fetch_all(&database)
        .await;

        match accounts_result {
            Ok(accounts) => {
                tracing::debug!("实时监听：找到 {} 个活跃账户", accounts.len());
                let mut has_errors = false;

                for account in accounts {
                    tracing::debug!("实时监听：检查账户 {} 的新邮件", account.name);

                    // 对QQ邮箱进行连接健康检查
                    if account.imap_server.contains("qq.com") {
                        match EmailService::qq_connection_health_check(&account).await {
                            Ok(false) => {
                                tracing::warn!("QQ邮箱 {} 连接不健康，跳过本次检查", account.name);
                                continue;
                            },
                            Err(e) => {
                                tracing::warn!("QQ邮箱 {} 健康检查失败: {}", account.name, e);
                                continue;
                            },
                            Ok(true) => {
                                tracing::debug!("QQ邮箱 {} 连接健康", account.name);
                            }
                        }
                    }

                    // 为每个账户检查新邮件
                    // 添加小延迟以确保时序正确，避免海森堡效应
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    match check_new_messages_realtime(&state, &account.id).await {
                        Ok(new_messages_count) => {
                            if new_messages_count > 0 {
                                tracing::info!("实时监听：账户 {} 收到 {} 封新邮件", account.name, new_messages_count);

                                // 发送事件到前端通知有新邮件
                                let _ = app_handle.emit("new-messages", serde_json::json!({
                                    "account_id": account.id,
                                    "account_name": account.name,
                                    "count": new_messages_count
                                }));
                            } else {
                                tracing::debug!("实时监听：账户 {} 没有新邮件", account.name);
                            }
                        },
                        Err(e) => {
                            has_errors = true;
                            // 改进错误处理：区分不同类型的错误
                            if e.contains("401") || e.contains("Unauthorized") || e.contains("token") {
                                tracing::warn!("实时监听：账户 {} 认证失败，可能需要重新授权: {}", account.name, e);
                            } else if e.contains("TLS握手失败") || e.contains("TLS") {
                                tracing::warn!("实时监听：账户 {} TLS握手失败，网络可能不稳定: {}", account.name, e);

                                // TLS握手失败时，给更长的恢复时间
                                if account.imap_server.contains("qq.com") {
                                    tracing::info!("QQ邮箱TLS握手失败，延长等待时间到10秒");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                                } else {
                                    tracing::info!("TLS握手失败，延长等待时间到5秒");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                }
                            } else if e.contains("网络") || e.contains("连接") || e.contains("timeout") || e.contains("TCP") {
                                tracing::warn!("实时监听：账户 {} 网络连接问题: {}", account.name, e);

                                // 特殊处理QQ邮箱的连接问题
                                if account.imap_server.contains("qq.com") {
                                    tracing::info!("检测到QQ邮箱连接问题，延长等待时间以避免频繁重连");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                }
                            } else {
                                tracing::warn!("实时监听：账户 {} IMAP获取邮件失败: {}", account.name, e);
                            }
                            // 继续处理其他账户，不让单个账户的错误影响整体监听
                        }
                    }

                    // 在账户之间添加小延迟，避免同时请求过多
                    // QQ邮箱使用更长的延迟时间
                    let delay = if account.imap_server.contains("qq.com") {
                        tokio::time::Duration::from_millis(500) // QQ邮箱500ms延迟
                    } else {
                        tokio::time::Duration::from_millis(100) // 其他邮箱100ms延迟
                    };
                    tokio::time::sleep(delay).await;
                }

                // 改进的错误恢复逻辑
                if has_errors {
                    consecutive_errors += 1;

                    // 检查是否是网络问题
                    if consecutive_errors >= 3 && last_network_check.elapsed() > std::time::Duration::from_secs(30) {
                        tracing::info!("检测到连续错误，开始网络连接检测...");
                        let network_available = check_network_connectivity().await;
                        last_network_check = std::time::Instant::now();

                        if !network_available && !network_down {
                            network_down = true;
                            tracing::warn!("🔴 网络连接已断开，暂停邮件同步，等待网络恢复...");
                        } else if network_available && network_down {
                            network_down = false;
                            consecutive_errors = 0; // 网络恢复时重置错误计数
                            tracing::info!("🟢 网络连接已恢复，重新开始邮件同步！");
                        }
                    }

                    if consecutive_errors >= max_consecutive_errors {
                        if network_down {
                            tracing::info!("网络断开状态，延长检查间隔到60秒");
                            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                        } else {
                            tracing::warn!("连续 {} 次检查出现错误，延长检查间隔到30秒", consecutive_errors);
                            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                        }
                        consecutive_errors = 0; // 重置计数器
                    }
                } else {
                    if network_down {
                        network_down = false;
                        tracing::info!("🟢 邮件同步已恢复正常！");
                    }
                    consecutive_errors = 0; // 重置错误计数器
                }
            },
            Err(e) => {
                consecutive_errors += 1;
                tracing::warn!("获取活跃账户失败 (第{}次): {}", consecutive_errors, e);

                if consecutive_errors >= max_consecutive_errors {
                    tracing::error!("连续 {} 次获取账户失败，延长检查间隔到60秒", consecutive_errors);
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                    consecutive_errors = 0; // 重置计数器
                }
            }
        }
    }
}

// 网络连接检测函数
async fn check_network_connectivity() -> bool {
    // 尝试连接多个知名服务器来检测网络状态
    let test_hosts = [
        ("8.8.8.8", 53),        // Google DNS
        ("1.1.1.1", 53),        // Cloudflare DNS
        ("114.114.114.114", 53), // 114 DNS
    ];

    for (host, port) in test_hosts.iter() {
        if let Ok(_) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect((*host, *port))
        ).await {
            tracing::debug!("网络连接检测成功: {}:{}", host, port);
            return true;
        }
    }

    tracing::warn!("网络连接检测失败：无法连接到任何测试服务器");
    false
}

// 为账户触发初始加载
async fn trigger_initial_load_for_account(
    state: &AppState,
    account_id: &str,
    account: &EmailAccount,
    folder: &str,
) -> Result<usize, String> {
    tracing::info!("实时监听：为账户 {} 触发初始加载", account.name);

    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
    };

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    // 获取初始邮件（120封）
    let messages = if account.imap_server.contains("gmail") && account.auth_type.as_deref() == Some("oauth2") {
        if let Some(access_token) = &account.access_token {
            match gmail_api::GmailApiService::fetch_messages(access_token, folder, Some(120)).await {
                Ok(mut messages) => {
                    for message in &mut messages {
                        message.account_id = account_id.to_string();
                    }
                    messages
                },
                Err(e) => {
                    tracing::error!("实时监听：Gmail API初始加载失败: {}", e);
                    return Ok(0);
                }
            }
        } else {
            return Ok(0);
        }
    } else if (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) && account.auth_type.as_deref() == Some("oauth2") {
        if let Some(access_token) = &account.access_token {
            match outlook_api::OutlookApiService::fetch_messages(access_token, folder, Some(120)).await {
                Ok(mut messages) => {
                    for message in &mut messages {
                        message.account_id = account_id.to_string();
                    }
                    messages
                },
                Err(e) => {
                    tracing::error!("实时监听：Outlook API初始加载失败: {}", e);
                    return Ok(0);
                }
            }
        } else {
            return Ok(0);
        }
    } else {
        match EmailService::fetch_messages_with_db(account, folder, Some(120), Some(&database)).await {
            Ok(messages) => messages,
            Err(e) => {
                tracing::error!("实时监听：IMAP初始加载失败: {}", e);
                return Ok(0);
            }
        }
    };

    // 保存到数据库
    match email_database.save_emails(&messages).await {
        Ok(saved_count) => {
            tracing::info!("实时监听：初始加载完成，保存了 {} 封邮件", saved_count);
            Ok(saved_count)
        },
        Err(e) => {
            tracing::error!("实时监听：初始加载保存失败: {}", e);
            Ok(0)
        }
    }
}

// 实时检查新邮件函数 - 使用增量更新，只获取真正的新邮件
async fn check_new_messages_realtime(
    state: &AppState,
    account_id: &str,
) -> Result<usize, String> {
    tracing::debug!("实时监听：开始检查账户 {} 的新邮件", account_id);

    // 检查实时监听是否被暂停
    if !*state.auto_sync_enabled.lock().unwrap() {
        tracing::debug!("实时监听已暂停，跳过检查");
        return Ok(0);
    }

    // 移除首次加载检查 - 所有账户都应该被监听
    // 如果账户没有邮件数据，实时监听会自动触发初始加载

    tracing::debug!("实时监听：账户 {} 已完成首次加载，继续检查", account_id);

    tracing::debug!("实时监听：获取数据库连接");
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
    };

    let email_database = {
        let guard = state.email_database.lock().unwrap();
        guard.as_ref().ok_or("SQLite邮件数据库未初始化")?.clone()
    };

    tracing::debug!("实时监听：数据库连接获取成功");

    // 获取账户信息
    tracing::debug!("实时监听：开始查询账户信息");
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ? AND is_active = TRUE"
    )
    .bind(account_id)
    .fetch_optional(&database)
    .await
    .map_err(|e| format!("查询账户失败: {}", e))?
    .ok_or("账户不存在或已禁用")?;

    tracing::debug!("实时监听：账户信息查询成功，账户名: {}", account.name);

    let folder = "INBOX";
    tracing::debug!("实时监听：开始映射文件夹: {}", folder);

    // 映射文件夹名称
    let actual_folder = match state.folder_mapper.map_folder(&account, folder).await {
        Ok(folder) => {
            tracing::debug!("实时监听：文件夹映射成功: {} -> {}", folder, folder);
            folder
        },
        Err(e) => {
            tracing::error!("实时监听：文件夹映射失败: {}", e);
            return Err(format!("文件夹映射失败: {}", e));
        },
    };

    // 获取数据库中最新邮件的时间戳，用于增量更新
    let latest_email_time = sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
        "SELECT MAX(received_at) FROM emails WHERE account_id = ? AND folder = ?"
    )
    .bind(account_id)
    .bind(&actual_folder)
    .fetch_one(email_database.pool())
    .await
    .unwrap_or(None);

    let db_email_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM emails WHERE account_id = ? AND folder = ?"
    )
    .bind(account_id)
    .bind(&actual_folder)
    .fetch_one(email_database.pool())
    .await
    .unwrap_or(0);

    tracing::debug!("实时监听：数据库状态 - 邮件总数: {}, 最新邮件时间: {:?}", db_email_count, latest_email_time);

    // 如果数据库为空，触发初始加载
    if db_email_count == 0 {
        tracing::info!("实时监听：账户 {} 数据库为空，触发初始加载", account.name);
        return trigger_initial_load_for_account(state, account_id, &account, &actual_folder).await;
    }

    // 使用增量更新策略：只获取最新的几封邮件进行检查
    tracing::debug!("实时监听：使用增量更新策略检查新邮件");

    let messages = if account.imap_server.contains("gmail") && account.auth_type.as_deref() == Some("oauth2") {
        tracing::debug!("实时监听：使用Gmail API检查新邮件，账户: {}", account.name);
        if let Some(access_token) = &account.access_token {
            tracing::debug!("实时监听：访问令牌存在，开始获取邮件");

            // 首先尝试使用当前token，只获取最新的5封邮件进行增量检查
            match gmail_api::GmailApiService::fetch_messages(access_token, &actual_folder, Some(5)).await {
                Ok(mut messages) => {
                    tracing::debug!("实时监听：Gmail API成功获取 {} 封邮件", messages.len());
                    // 设置account_id
                    for message in &mut messages {
                        message.account_id = account_id.to_string();
                    }
                    messages
                },
                Err(e) => {
                    tracing::debug!("实时监听：Gmail API获取邮件失败: {}", e);

                    // 如果是401错误，尝试刷新令牌
                    if e.to_string().contains("401") || e.to_string().contains("Unauthorized") {
                        tracing::info!("实时监听：检测到令牌过期，尝试刷新OAuth2访问令牌");

                        if let Some(refresh_token) = &account.refresh_token {
                            // 根据账户类型选择正确的OAuth2配置和刷新方法
                            let refresh_result = if account.imap_server.contains("outlook") || account.imap_server.contains("office365") {
                                // Outlook账户
                                let config = get_outlook_oauth2_config();
                                let client = OAuth2Client::new(config);
                                client.refresh_outlook_token(refresh_token).await
                            } else {
                                // Gmail账户
                                let config = get_gmail_oauth2_config();
                                let client = OAuth2Client::new(config);
                                client.refresh_token(refresh_token).await
                            };

                            match refresh_result {
                                Ok(new_token) => {
                                    tracing::info!("实时监听：OAuth2令牌刷新成功");

                                    // 更新数据库中的访问令牌
                                    if let Err(e) = update_oauth2_token_internal(&database, account_id, &new_token.access_token).await {
                                        tracing::error!("实时监听：更新访问令牌失败: {}", e);
                                        return Ok(0);
                                    }

                                    tracing::info!("实时监听：访问令牌已更新到数据库");

                                    // 使用新令牌重试
                                    match gmail_api::GmailApiService::fetch_messages(&new_token.access_token, &actual_folder, Some(5)).await {
                                        Ok(mut messages) => {
                                            tracing::debug!("实时监听：使用新令牌成功获取 {} 封邮件", messages.len());
                                            // 设置account_id
                                            for message in &mut messages {
                                                message.account_id = account_id.to_string();
                                            }
                                            messages
                                        },
                                        Err(e) => {
                                            tracing::error!("实时监听：使用新令牌仍然失败: {}", e);
                                            return Ok(0);
                                        }
                                    }
                                },
                                Err(e) => {
                                    tracing::error!("实时监听：OAuth2令牌刷新失败: {}", e);
                                    return Ok(0);
                                }
                            }
                        } else {
                            tracing::error!("实时监听：没有刷新令牌，无法刷新访问令牌");
                            return Ok(0);
                        }
                    } else {
                        return Ok(0);
                    }
                }
            }
        } else {
            tracing::warn!("Gmail账户缺少访问令牌");
            return Ok(0);
        }
    } else if (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) && account.auth_type.as_deref() == Some("oauth2") {
        // 使用Outlook Graph API
        tracing::debug!("实时监听：使用Outlook Graph API检查新邮件，账户: {}", account.name);
        if let Some(access_token) = &account.access_token {
            tracing::debug!("实时监听：Outlook访问令牌存在，开始获取邮件");
            tracing::debug!("实时监听：访问令牌长度: {}, 前20字符: {}", access_token.len(), &access_token[..access_token.len().min(20)]);
            tracing::debug!("实时监听：开始调用Outlook Graph API");
            match outlook_api::OutlookApiService::fetch_messages(access_token, &actual_folder, Some(5)).await {
                Ok(mut messages) => {
                    tracing::debug!("实时监听：Outlook Graph API成功获取 {} 封邮件", messages.len());
                    // 设置account_id
                    for message in &mut messages {
                        message.account_id = account_id.to_string();
                    }
                    messages
                },
                Err(e) => {
                    let error_str = e.to_string();
                    tracing::debug!("实时监听：Outlook Graph API获取邮件失败: {}", error_str);

                    // 如果是401错误，尝试刷新令牌
                    if error_str.contains("401") || error_str.contains("Unauthorized") {
                        tracing::info!("实时监听：检测到令牌过期，尝试刷新OAuth2访问令牌");

                        if let Some(refresh_token) = &account.refresh_token {
                            // 刷新Outlook令牌
                            let config = get_outlook_oauth2_config();
                            let client = OAuth2Client::new(config);
                            match client.refresh_outlook_token(refresh_token).await {
                                Ok(new_token) => {
                                    tracing::info!("实时监听：成功刷新Outlook访问令牌");

                                    // 更新数据库中的令牌
                                    if let Err(e) = update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await {
                                        tracing::error!("实时监听：更新数据库中的令牌失败: {}", e);
                                        return Ok(0);
                                    }

                                    // 使用新令牌重试获取邮件
                                    match outlook_api::OutlookApiService::fetch_messages(&new_token.access_token, &actual_folder, Some(5)).await {
                                        Ok(mut messages) => {
                                            tracing::debug!("实时监听：使用新令牌成功获取 {} 封邮件", messages.len());

                                            // 按接收时间排序（最新的在前）
                                            messages.sort_by(|a, b| b.received_at.cmp(&a.received_at));
                                            messages
                                        },
                                        Err(retry_e) => {
                                            tracing::error!("实时监听：使用新令牌获取邮件仍然失败: {}", retry_e);
                                            return Ok(0);
                                        }
                                    }
                                },
                                Err(refresh_e) => {
                                    tracing::error!("实时监听：刷新令牌失败: {}", refresh_e);
                                    return Ok(0);
                                }
                            }
                        } else {
                            tracing::warn!("实时监听：账户没有刷新令牌，无法自动刷新");
                            return Ok(0);
                        }
                    } else {
                        return Ok(0);
                    }
                }
            }
        } else {
            tracing::info!("实时监听：Outlook账户缺少访问令牌");
            return Ok(0);
        }
    } else {
        tracing::debug!("实时监听：使用IMAP检查新邮件，账户: {}", account.name);
        // 使用传统IMAP方式
        match EmailService::fetch_messages(&account, &actual_folder, Some(5)).await {
            Ok(messages) => {
                tracing::debug!("实时监听：IMAP成功获取 {} 封邮件", messages.len());
                messages
            },
            Err(e) => {
                tracing::warn!("实时监听：IMAP获取邮件失败: {}", e);
                return Ok(0);
            }
        }
    };

    if messages.is_empty() {
        tracing::debug!("实时监听：没有获取到任何邮件");
        return Ok(0);
    }

    tracing::debug!("实时监听：开始检查 {} 封邮件是否为新邮件", messages.len());

    if messages.is_empty() {
        tracing::debug!("实时监听：没有获取到任何邮件，跳过检查");
        return Ok(0);
    }

    // 检查这些邮件中是否有新邮件（不在数据库中的）
    let mut new_messages = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        // 检查邮件是否已存在
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE account_id = ? AND message_id = ?"
        )
        .bind(account_id)
        .bind(&message.message_id)
        .fetch_one(email_database.pool())
        .await
        .unwrap_or(0) > 0;

        tracing::debug!("实时监听：邮件 {}/{} - {} - {}",
            index + 1, messages.len(),
            message.message_id,
            if exists { "已存在" } else { "新邮件" }
        );

        if !exists {
            new_messages.push(message.clone());
        }
    }

    tracing::debug!("实时监听：检查完成，发现 {} 封新邮件", new_messages.len());

    if !new_messages.is_empty() {
        match email_database.save_emails(&new_messages).await {
            Ok(saved_count) => {
                tracing::info!("实时监听保存 {} 封新邮件", saved_count);
                Ok(saved_count)
            },
            Err(e) => {
                tracing::warn!("保存新邮件失败: {}", e);
                Ok(0)
            }
        }
    } else {
        // 只在第一次检查时显示，避免日志过多
        static mut FIRST_CHECK: bool = true;
        unsafe {
            if FIRST_CHECK {
                tracing::info!("实时监听正常运行，等待新邮件...");
                FIRST_CHECK = false;
            }
        }
        Ok(0)
    }
}

// OAuth2 相关命令
#[tauri::command]
async fn get_gmail_oauth_url() -> Result<String, String> {
    let config = get_gmail_oauth2_config();
    let client = OAuth2Client::new(config);

    match client.get_authorization_url(None) {
        Ok(url) => Ok(url),
        Err(e) => {
            tracing::error!("生成OAuth2授权URL失败: {}", e);
            Err(format!("生成OAuth2授权URL失败: {}", e))
        }
    }
}

#[tauri::command]
async fn exchange_oauth_code(code: String) -> Result<OAuth2Token, String> {
    let config = get_gmail_oauth2_config();
    let client = OAuth2Client::new(config);

    match client.exchange_code_for_token(&code).await {
        Ok(token) => {
            tracing::info!("OAuth2令牌交换成功");
            Ok(token)
        },
        Err(e) => {
            tracing::error!("OAuth2令牌交换失败: {}", e);
            Err(format!("OAuth2令牌交换失败: {}", e))
        }
    }
}

#[tauri::command]
async fn refresh_oauth_token(refresh_token: String) -> Result<OAuth2Token, String> {
    let config = get_gmail_oauth2_config();
    let client = OAuth2Client::new(config);

    match client.refresh_token(&refresh_token).await {
        Ok(token) => {
            tracing::info!("OAuth2令牌刷新成功");
            Ok(token)
        },
        Err(e) => {
            tracing::error!("OAuth2令牌刷新失败: {}", e);
            Err(format!("OAuth2令牌刷新失败: {}", e))
        }
    }
}

#[tauri::command]
async fn verify_oauth_token(access_token: String) -> Result<bool, String> {
    match verify_gmail_token(&access_token).await {
        Ok(valid) => Ok(valid),
        Err(e) => {
            tracing::error!("验证OAuth2令牌失败: {}", e);
            Err(format!("验证OAuth2令牌失败: {}", e))
        }
    }
}

#[tauri::command]
async fn refresh_account_token(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<String, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("MySQL数据库未初始化")?.pool().clone()
    };

    // 获取账户信息
    let account = sqlx::query_as::<_, EmailAccount>(
        "SELECT * FROM email_accounts WHERE id = ?"
    )
    .bind(&account_id)
    .fetch_one(&database)
    .await
    .map_err(|e| format!("获取账户信息失败: {}", e))?;

    if let Some(refresh_token) = &account.refresh_token {
        // 根据账户类型选择正确的OAuth2配置和刷新方法
        let refresh_result = if account.imap_server.contains("outlook") || account.imap_server.contains("office365") {
            // Outlook账户
            let config = get_outlook_oauth2_config();
            let client = OAuth2Client::new(config);
            client.refresh_outlook_token(refresh_token).await
        } else {
            // Gmail账户
            let config = get_gmail_oauth2_config();
            let client = OAuth2Client::new(config);
            client.refresh_token(refresh_token).await
        };

        match refresh_result {
            Ok(new_token) => {
                tracing::info!("成功刷新访问令牌，账户: {}", account_id);

                // 更新数据库中的令牌
                update_oauth2_token_internal(&database, &account_id, &new_token.access_token).await
                    .map_err(|e| format!("更新数据库中的令牌失败: {}", e))?;

                Ok("令牌刷新成功".to_string())
            },
            Err(e) => {
                tracing::error!("刷新令牌失败: {}", e);
                Err(format!("刷新令牌失败: {}", e))
            }
        }
    } else {
        Err("账户没有刷新令牌".to_string())
    }
}

#[tauri::command]
async fn complete_gmail_oauth2() -> Result<OAuth2Token, String> {
    match complete_oauth2_flow().await {
        Ok(token) => {
            tracing::info!("Gmail OAuth2授权完成");
            Ok(token)
        },
        Err(e) => {
            tracing::error!("Gmail OAuth2授权失败: {}", e);
            Err(format!("Gmail OAuth2授权失败: {}", e))
        }
    }
}

#[tauri::command]
async fn start_outlook_oauth2_window(app_handle: tauri::AppHandle) -> Result<OAuth2Token, String> {
    use tauri::{Manager, WebviewWindowBuilder};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    tracing::info!("开始Outlook OAuth2授权流程");

    let config = get_outlook_oauth2_config();
    let client = OAuth2Client::new(config);

    // 存储授权码的共享变量
    let auth_code = Arc::new(Mutex::new(None::<String>));
    let _auth_code_clone = auth_code.clone();

    // 启动本地HTTP服务器来处理OAuth2回调
    let auth_code_for_server = auth_code.clone();
    let server_task = tokio::spawn(async move {
        use warp::Filter;
        use std::collections::HashMap;

        let callback_route = warp::path("auth")
            .and(warp::path("callback"))
            .and(warp::path::end())
            .and(warp::query::<HashMap<String, String>>())
            .map(move |params: HashMap<String, String>| {
                if let Some(code) = params.get("code") {
                    // 存储授权码
                    if let Ok(mut guard) = auth_code_for_server.lock() {
                        *guard = Some(code.clone());
                    }

                    warp::reply::html(
                        r#"
                        <html>
                            <head><title>Outlook授权成功</title></head>
                            <body>
                                <script>
                                    // 立即尝试关闭窗口
                                    try {
                                        window.close();
                                    } catch (e) {
                                        try {
                                            window.open('', '_self');
                                            window.close();
                                        } catch (e2) {
                                            window.location.href = 'about:blank';
                                        }
                                    }
                                </script>
                            </body>
                        </html>
                        "#.to_string()
                    )
                } else {
                    warp::reply::html("授权失败".to_string())
                }
            });

        warp::serve(callback_route)
            .run(([127, 0, 0, 1], 8081))
            .await;
    });

    // 生成授权URL（使用Outlook专用方法）
    let auth_url = client.get_outlook_authorization_url(Some("outlook_oauth2_state".to_string()))
        .map_err(|e| format!("生成授权URL失败: {}", e))?;

    // 创建OAuth2授权弹窗
    let _oauth_window = WebviewWindowBuilder::new(
        &app_handle,
        "outlook-oauth2-popup",
        tauri::WebviewUrl::External(auth_url.parse().unwrap())
    )
    .title("Outlook 授权")
    .inner_size(500.0, 700.0)
    .center()
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .build()
    .map_err(|e| format!("创建Outlook OAuth2弹窗失败: {}", e))?;

    tracing::info!("Outlook OAuth2授权弹窗已创建");

    // 等待授权码或超时
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(120); // 2分钟超时

    let code = loop {
        // 检查是否收到授权码
        if let Ok(guard) = auth_code.lock() {
            if let Some(code) = guard.as_ref() {
                // 收到授权码，关闭弹窗和停止服务器
                if let Some(window) = app_handle.get_webview_window("outlook-oauth2-popup") {
                    let _ = window.close();
                    tracing::info!("Outlook OAuth2弹窗已关闭");
                }
                server_task.abort();
                break code.clone();
            }
        }

        // 检查窗口是否还存在
        if app_handle.get_webview_window("outlook-oauth2-popup").is_none() {
            server_task.abort();
            return Err("授权弹窗已关闭".to_string());
        }

        // 检查超时
        if start_time.elapsed() > timeout {
            // 关闭弹窗和服务器
            server_task.abort();
            if let Some(window) = app_handle.get_webview_window("outlook-oauth2-popup") {
                let _ = window.close();
            }
            return Err("授权超时（2分钟），请重试".to_string());
        }

        // 短暂休眠避免忙等待
        tokio::time::sleep(Duration::from_millis(200)).await;
    };

    // 使用授权码交换访问令牌
    let token = client.exchange_code_for_token(&code).await
        .map_err(|e| format!("令牌交换失败: {}", e))?;

    tracing::info!("Outlook OAuth2授权完成，令牌交换成功");
    Ok(token)
}

#[tauri::command]
async fn start_oauth2_window(app_handle: tauri::AppHandle) -> Result<OAuth2Token, String> {
    use tauri::{Manager, WebviewWindowBuilder};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    let config = get_gmail_oauth2_config();
    let client = OAuth2Client::new(config);

    // 存储授权码的共享变量
    let auth_code = Arc::new(Mutex::new(None::<String>));
    let _auth_code_clone = auth_code.clone();

    // 启动本地HTTP服务器来处理OAuth2回调
    let auth_code_for_server = auth_code.clone();
    let server_task = tokio::spawn(async move {
        use warp::Filter;
        use std::collections::HashMap;

        let callback_route = warp::path::end()
            .and(warp::query::<HashMap<String, String>>())
            .map(move |params: HashMap<String, String>| {
                if let Some(code) = params.get("code") {
                    // 存储授权码
                    if let Ok(mut guard) = auth_code_for_server.lock() {
                        *guard = Some(code.clone());
                    }

                    warp::reply::html(
                        r#"
                        <html>
                            <head><title>Gmail授权成功</title></head>
                            <body>
                                <script>
                                    // 立即尝试关闭窗口
                                    try {
                                        window.close();
                                    } catch (e) {
                                        try {
                                            window.open('', '_self');
                                            window.close();
                                        } catch (e2) {
                                            window.location.href = 'about:blank';
                                        }
                                    }
                                </script>
                            </body>
                        </html>
                        "#.to_string()
                    )
                } else {
                    warp::reply::html("授权失败".to_string())
                }
            });

        // 使用与邮件功能相同的端口
        let port = 8081;

        warp::serve(callback_route)
            .run(([127, 0, 0, 1], port))
            .await;
    });

    // 生成授权URL
    let auth_url = client.get_authorization_url(None)
        .map_err(|e| format!("生成授权URL失败: {}", e))?;

    // 创建OAuth2授权弹窗
    let _oauth_window = WebviewWindowBuilder::new(
        &app_handle,
        "oauth2-popup",
        tauri::WebviewUrl::External(auth_url.parse().unwrap())
    )
    .title("Gmail 授权")
    .inner_size(500.0, 700.0)
    .center()
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .build()
    .map_err(|e| format!("创建OAuth2弹窗失败: {}", e))?;

    tracing::info!("OAuth2授权弹窗已创建");

    // 等待授权码或超时
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(120); // 2分钟超时

    let code = loop {
        // 检查是否收到授权码
        if let Ok(guard) = auth_code.lock() {
            if let Some(code) = guard.as_ref() {
                // 收到授权码，关闭弹窗和停止服务器
                if let Some(window) = app_handle.get_webview_window("oauth2-popup") {
                    let _ = window.close();
                    tracing::info!("OAuth2弹窗已关闭");
                }
                server_task.abort();
                break code.clone();
            }
        }

        // 检查窗口是否还存在
        if app_handle.get_webview_window("oauth2-popup").is_none() {
            server_task.abort();
            return Err("授权弹窗已关闭".to_string());
        }

        // 检查超时
        if start_time.elapsed() > timeout {
            // 关闭弹窗和服务器
            server_task.abort();
            if let Some(window) = app_handle.get_webview_window("oauth2-popup") {
                let _ = window.close();
            }
            return Err("授权超时（2分钟），请重试".to_string());
        }

        // 短暂休眠避免忙等待
        tokio::time::sleep(Duration::from_millis(200)).await;
    };

    // 使用授权码交换访问令牌
    let token = client.exchange_code_for_token(&code).await
        .map_err(|e| format!("令牌交换失败: {}", e))?;

    tracing::info!("OAuth2授权完成，令牌交换成功");
    Ok(token)
}

// 定期令牌刷新任务
async fn periodic_token_refresh(app_handle: tauri::AppHandle) {
    tracing::info!("定期令牌刷新任务启动，每30分钟检查一次令牌状态");

    let mut interval = time::interval(Duration::from_secs(30 * 60)); // 每30分钟检查一次

    loop {
        interval.tick().await;

        tracing::debug!("开始定期令牌刷新检查");

        // 获取应用状态
        let state = app_handle.state::<AppState>();

        // 获取数据库连接
        let database = {
            let guard = state.database.lock().unwrap();
            match guard.as_ref() {
                Some(db) => db.pool().clone(),
                None => {
                    tracing::warn!("数据库未初始化，跳过令牌刷新检查");
                    continue;
                }
            }
        };

        // 获取所有OAuth2账户
        let accounts_result = sqlx::query_as::<_, EmailAccount>(
            "SELECT * FROM email_accounts WHERE is_active = TRUE AND refresh_token IS NOT NULL"
        )
        .fetch_all(&database)
        .await;

        let accounts = match accounts_result {
            Ok(accounts) => accounts,
            Err(e) => {
                tracing::error!("获取账户列表失败: {}", e);
                continue;
            }
        };

        if accounts.is_empty() {
            tracing::debug!("没有需要刷新令牌的OAuth2账户");
            continue;
        }

        tracing::info!("开始为 {} 个OAuth2账户刷新令牌", accounts.len());

        for account in accounts {
            if let Some(refresh_token) = &account.refresh_token {
                // 根据账户类型选择正确的OAuth2配置和刷新方法
                let refresh_result = if account.imap_server.contains("outlook") || account.imap_server.contains("office365") {
                    // Outlook账户
                    let config = get_outlook_oauth2_config();
                    let client = OAuth2Client::new(config);
                    client.refresh_outlook_token(refresh_token).await
                } else {
                    // Gmail账户
                    let config = get_gmail_oauth2_config();
                    let client = OAuth2Client::new(config);
                    client.refresh_token(refresh_token).await
                };

                match refresh_result {
                    Ok(new_token) => {
                        tracing::info!("定期刷新成功，账户: {} ({})", account.name, account.email);

                        // 更新数据库中的令牌
                        if let Err(e) = update_oauth2_token_internal(&database, &account.id, &new_token.access_token).await {
                            tracing::error!("更新数据库中的令牌失败: {}", e);
                        }
                    },
                    Err(e) => {
                        tracing::warn!("定期刷新失败，账户: {} ({}), 错误: {}", account.name, account.email, e);
                    }
                }

                // 在账户之间添加短暂延迟，避免API限制
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        tracing::info!("定期令牌刷新检查完成");
    }
}

// 联系人管理命令
#[tauri::command]
async fn create_contact(
    state: State<'_, AppState>,
    contact_data: NewContact
) -> Result<Contact, String> {
    // 确保数据库已初始化
    let database = {
        let needs_init = {
            let guard = state.database.lock().unwrap();
            guard.is_none()
        };

        if needs_init {
            tracing::info!("数据库未初始化，开始初始化数据库...");
            match Database::new().await {
                Ok(db) => {
                    tracing::info!("数据库初始化成功");
                    let mut guard = state.database.lock().unwrap();
                    *guard = Some(db);
                }
                Err(e) => {
                    tracing::error!("数据库初始化失败: {}", e);
                    return Err(format!("数据库初始化失败: {}", e));
                }
            }
        }

        let guard = state.database.lock().unwrap();
        guard.as_ref().unwrap().pool().clone()
    };

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    // 插入联系人到数据库
    sqlx::query(
        r#"
        INSERT INTO contacts (id, user_id, name, email, phone, company, notes, avatar, is_favorite, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&contact_data.user_id)
    .bind(&contact_data.name)
    .bind(&contact_data.email)
    .bind(&contact_data.phone)
    .bind(&contact_data.company)
    .bind(&contact_data.notes)
    .bind(&contact_data.avatar)
    .bind(contact_data.is_favorite)
    .bind(now)
    .bind(now)
    .execute(&database)
    .await
    .map_err(|e| {
        tracing::error!("创建联系人失败: {}", e);
        format!("创建联系人失败: {}", e)
    })?;

    tracing::info!("联系人创建成功，ID: {}", id);

    Ok(Contact {
        id,
        user_id: contact_data.user_id,
        name: contact_data.name,
        email: contact_data.email,
        phone: contact_data.phone,
        company: contact_data.company,
        notes: contact_data.notes,
        avatar: contact_data.avatar,
        is_favorite: contact_data.is_favorite,
        created_at: now,
        updated_at: now,
    })
}

#[tauri::command]
async fn get_contacts(
    state: State<'_, AppState>,
    user_id: String
) -> Result<Vec<Contact>, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    let contacts = sqlx::query_as::<_, Contact>(
        "SELECT * FROM contacts WHERE user_id = ? ORDER BY name ASC"
    )
    .bind(&user_id)
    .fetch_all(&database)
    .await
    .map_err(|e| {
        tracing::error!("获取联系人列表失败: {}", e);
        format!("获取联系人列表失败: {}", e)
    })?;

    tracing::info!("获取到 {} 个联系人", contacts.len());
    Ok(contacts)
}

#[tauri::command]
async fn update_contact(
    state: State<'_, AppState>,
    contact_id: String,
    contact_data: NewContact
) -> Result<String, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    let now = chrono::Utc::now();

    sqlx::query(
        r#"
        UPDATE contacts
        SET name = ?, email = ?, phone = ?, company = ?, notes = ?, avatar = ?, is_favorite = ?, updated_at = ?
        WHERE id = ? AND user_id = ?
        "#,
    )
    .bind(&contact_data.name)
    .bind(&contact_data.email)
    .bind(&contact_data.phone)
    .bind(&contact_data.company)
    .bind(&contact_data.notes)
    .bind(&contact_data.avatar)
    .bind(contact_data.is_favorite)
    .bind(now)
    .bind(&contact_id)
    .bind(&contact_data.user_id)
    .execute(&database)
    .await
    .map_err(|e| {
        tracing::error!("更新联系人失败: {}", e);
        format!("更新联系人失败: {}", e)
    })?;

    tracing::info!("联系人更新成功，ID: {}", contact_id);
    Ok("联系人更新成功".to_string())
}

#[tauri::command]
async fn delete_contact(
    state: State<'_, AppState>,
    contact_id: String,
    user_id: String
) -> Result<String, String> {
    let database = {
        let guard = state.database.lock().unwrap();
        guard.as_ref().ok_or("数据库未初始化")?.pool().clone()
    };

    sqlx::query(
        "DELETE FROM contacts WHERE id = ? AND user_id = ?"
    )
    .bind(&contact_id)
    .bind(&user_id)
    .execute(&database)
    .await
    .map_err(|e| {
        tracing::error!("删除联系人失败: {}", e);
        format!("删除联系人失败: {}", e)
    })?;

    tracing::info!("联系人删除成功，ID: {}", contact_id);
    Ok("联系人删除成功".to_string())
}


