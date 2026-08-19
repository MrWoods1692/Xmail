use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// 用户模型
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String, // 使用String存储UUID
    pub username: String,
    pub email: String,
    pub password_hash: String, // 加密后的密码
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
    pub avatar: Option<String>, // 用户头像URL
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password: String, // 明文密码，会在后端加密
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub user: Option<UserInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub avatar: Option<String>, // 用户头像URL
}



#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailAccount {
    pub id: String, // 使用String存储UUID
    pub name: String,
    pub email: String,
    pub imap_server: String,
    pub imap_port: u16,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String, // 注意：实际应用中应该加密存储
    pub use_tls: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
    // OAuth2 相关字段
    pub auth_type: Option<String>, // "password" 或 "oauth2"
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEmailAccount {
    pub name: String,
    pub email: String,
    pub imap_server: String,
    pub imap_port: u16,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
    // OAuth2 相关字段
    pub auth_type: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailMessage {
    pub id: String, // 使用String存储UUID
    pub account_id: String, // 使用String存储UUID
    pub message_id: String, // 邮件服务器的消息ID
    pub subject: String,
    pub sender: String,
    pub recipients: String, // JSON格式存储收件人列表
    pub cc: Option<String>, // JSON格式存储抄送列表
    pub bcc: Option<String>, // JSON格式存储密送列表
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub folder: String, // INBOX, SENT, DRAFT, TRASH等
    pub is_read: bool,
    pub is_starred: bool,
    pub is_deleted: bool,
    pub received_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub imap_uid: Option<i64>, // IMAP UID for deletion
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEmailMessage {
    pub account_id: String, // 使用String存储UUID
    pub message_id: String,
    pub subject: String,
    pub sender: String,
    pub recipients: String,
    pub cc: Option<String>,
    pub bcc: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub folder: String,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailAttachment {
    pub id: String, // 使用String存储UUID
    pub message_id: String, // 使用String存储UUID
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub file_path: String, // 本地存储路径
    pub created_at: DateTime<Utc>,
}

// 联系人模型
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Contact {
    pub id: String, // 使用String存储UUID
    pub user_id: String, // 关联到用户
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub notes: Option<String>,
    pub avatar: Option<String>, // 头像URL
    pub is_favorite: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewContact {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub notes: Option<String>,
    pub avatar: Option<String>,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEmailAttachment {
    pub message_id: String, // 使用String存储UUID
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailFolder {
    pub id: String, // 使用String存储UUID
    pub account_id: String, // 使用String存储UUID
    pub name: String,
    pub display_name: String,
    pub folder_type: String, // INBOX, SENT, DRAFT, TRASH, CUSTOM
    pub unread_count: i32,
    pub total_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEmailFolder {
    pub account_id: String, // 使用String存储UUID
    pub name: String,
    pub display_name: String,
    pub folder_type: String,
}
