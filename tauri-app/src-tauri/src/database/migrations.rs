use sqlx::MySqlPool;
use anyhow::Result;

pub async fn run_migrations(pool: &MySqlPool) -> Result<()> {
    tracing::info!("运行数据库迁移...");

    // 创建用户表
    create_users_table(pool).await?;

    // 创建邮箱账户表
    create_email_accounts_table(pool).await?;

    // 创建邮件消息表
    create_email_messages_table(pool).await?;

    // 创建邮件附件表
    create_email_attachments_table(pool).await?;

    // 创建邮件文件夹表
    create_email_folders_table(pool).await?;

    // 添加OAuth2字段到邮箱账户表
    add_oauth2_fields_to_accounts(pool).await?;

    // 添加avatar字段到用户表
    add_avatar_field_to_users(pool).await?;

    // 创建联系人表
    create_contacts_table(pool).await?;

    // 优化数据库索引
    optimize_database_indexes(pool).await?;

    tracing::info!("数据库迁移完成");
    Ok(())
}

async fn create_users_table(pool: &MySqlPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id CHAR(36) PRIMARY KEY,
            username VARCHAR(50) NOT NULL UNIQUE,
            email VARCHAR(255) NOT NULL UNIQUE,
            password_hash VARCHAR(255) NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            INDEX idx_username (username),
            INDEX idx_email (email),
            INDEX idx_active (is_active)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_email_accounts_table(pool: &MySqlPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS email_accounts (
            id CHAR(36) PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255) NOT NULL UNIQUE,
            imap_server VARCHAR(255) NOT NULL,
            imap_port INT UNSIGNED NOT NULL,
            smtp_server VARCHAR(255) NOT NULL,
            smtp_port INT UNSIGNED NOT NULL,
            username VARCHAR(255) NOT NULL,
            password TEXT NOT NULL,
            use_tls BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            INDEX idx_email (email),
            INDEX idx_active (is_active)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

async fn create_email_messages_table(pool: &MySqlPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS email_messages (
            id CHAR(36) PRIMARY KEY,
            account_id CHAR(36) NOT NULL,
            message_id VARCHAR(255) NOT NULL,
            subject TEXT NOT NULL,
            sender VARCHAR(255) NOT NULL,
            recipients TEXT NOT NULL,
            cc TEXT,
            bcc TEXT,
            body_text LONGTEXT,
            body_html LONGTEXT,
            folder VARCHAR(100) NOT NULL DEFAULT 'INBOX',
            is_read BOOLEAN NOT NULL DEFAULT FALSE,
            is_starred BOOLEAN NOT NULL DEFAULT FALSE,
            is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
            received_at TIMESTAMP NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (account_id) REFERENCES email_accounts(id) ON DELETE CASCADE,
            UNIQUE KEY unique_message (account_id, message_id),
            INDEX idx_account_folder (account_id, folder),
            INDEX idx_received_at (received_at),
            INDEX idx_is_read (is_read),
            INDEX idx_is_starred (is_starred),
            INDEX idx_is_deleted (is_deleted)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

async fn create_email_attachments_table(pool: &MySqlPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS email_attachments (
            id CHAR(36) PRIMARY KEY,
            message_id CHAR(36) NOT NULL,
            filename VARCHAR(255) NOT NULL,
            content_type VARCHAR(100) NOT NULL,
            size BIGINT NOT NULL,
            file_path TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (message_id) REFERENCES email_messages(id) ON DELETE CASCADE,
            INDEX idx_message_id (message_id)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

async fn create_email_folders_table(pool: &MySqlPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS email_folders (
            id CHAR(36) PRIMARY KEY,
            account_id CHAR(36) NOT NULL,
            name VARCHAR(255) NOT NULL,
            display_name VARCHAR(255) NOT NULL,
            folder_type VARCHAR(50) NOT NULL,
            unread_count INT NOT NULL DEFAULT 0,
            total_count INT NOT NULL DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (account_id) REFERENCES email_accounts(id) ON DELETE CASCADE,
            UNIQUE KEY unique_folder (account_id, name),
            INDEX idx_account_type (account_id, folder_type)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

async fn add_oauth2_fields_to_accounts(pool: &MySqlPool) -> Result<()> {
    // 检查是否已经存在OAuth2字段
    let column_exists = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM INFORMATION_SCHEMA.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
        AND TABLE_NAME = 'email_accounts'
        AND COLUMN_NAME = 'auth_type'
        "#
    )
    .fetch_one(pool)
    .await?;

    if column_exists == 0 {
        tracing::info!("添加OAuth2字段到email_accounts表...");

        // 添加OAuth2相关字段
        sqlx::query(
            r#"
            ALTER TABLE email_accounts
            ADD COLUMN auth_type VARCHAR(20) DEFAULT 'password',
            ADD COLUMN access_token TEXT NULL,
            ADD COLUMN refresh_token TEXT NULL,
            ADD COLUMN token_expires_at TIMESTAMP NULL
            "#
        )
        .execute(pool)
        .await?;

        tracing::info!("OAuth2字段添加完成");
    } else {
        tracing::info!("OAuth2字段已存在，跳过迁移");
    }

    Ok(())
}

async fn add_avatar_field_to_users(pool: &MySqlPool) -> Result<()> {
    // 检查是否已经存在avatar字段
    let column_exists = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM INFORMATION_SCHEMA.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
        AND TABLE_NAME = 'users'
        AND COLUMN_NAME = 'avatar'
        "#
    )
    .fetch_one(pool)
    .await?;

    if column_exists == 0 {
        tracing::info!("添加avatar字段到users表...");

        // 添加avatar字段
        sqlx::query(
            r#"
            ALTER TABLE users
            ADD COLUMN avatar TEXT NULL
            "#
        )
        .execute(pool)
        .await?;

        tracing::info!("avatar字段添加完成");
    } else {
        tracing::info!("avatar字段已存在，跳过迁移");
    }

    Ok(())
}

async fn optimize_database_indexes(pool: &MySqlPool) -> Result<()> {
    tracing::info!("优化数据库索引...");

    // 检查并添加email_accounts表的复合索引
    let index_exists = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM INFORMATION_SCHEMA.STATISTICS
        WHERE TABLE_SCHEMA = DATABASE()
        AND TABLE_NAME = 'email_accounts'
        AND INDEX_NAME = 'idx_active_email'
        "#
    )
    .fetch_one(pool)
    .await?;

    if index_exists == 0 {
        tracing::info!("添加email_accounts复合索引...");
        sqlx::query(
            r#"
            CREATE INDEX idx_active_email ON email_accounts (is_active, email)
            "#
        )
        .execute(pool)
        .await?;
        tracing::info!("email_accounts复合索引添加完成");
    } else {
        tracing::info!("email_accounts复合索引已存在，跳过创建");
    }

    tracing::info!("数据库索引优化完成");
    Ok(())
}

// 创建联系人表
async fn create_contacts_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    tracing::info!("检查联系人表...");

    // 检查表是否存在
    let table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'contacts'"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0) > 0;

    if !table_exists {
        tracing::info!("联系人表不存在，创建新表...");
        sqlx::query(
            r#"
            CREATE TABLE contacts (
                id VARCHAR(36) PRIMARY KEY,
                user_id VARCHAR(36) NOT NULL,
                name VARCHAR(255) NOT NULL,
                email VARCHAR(255) NOT NULL,
                phone VARCHAR(50),
                company VARCHAR(255),
                notes TEXT,
                avatar TEXT,
                is_favorite BOOLEAN NOT NULL DEFAULT FALSE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                INDEX idx_user_id (user_id),
                INDEX idx_email (email),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
            "#,
        )
        .execute(pool)
        .await?;
        tracing::info!("联系人表创建成功");
    } else {
        tracing::info!("联系人表已存在，跳过创建");
    }

    Ok(())
}


