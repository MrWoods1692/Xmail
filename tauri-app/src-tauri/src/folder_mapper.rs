use std::collections::HashMap;
use anyhow::{Result, Context};
use std::sync::Mutex;
use crate::database::models::EmailAccount;
use async_imap::Client;
use async_std::net::TcpStream;
use async_native_tls::TlsConnector;

pub struct FolderMapper {
    // 标准文件夹名称到实际文件夹名称的映射
    folder_mappings: HashMap<String, Vec<String>>,
    // 缓存账户的文件夹映射，避免重复查询服务器
    account_folder_cache: Mutex<HashMap<String, HashMap<String, String>>>,
}

impl FolderMapper {
    pub fn new() -> Self {
        let mut folder_mappings = HashMap::new();
        
        // 收件箱的可能名称
        folder_mappings.insert("INBOX".to_string(), vec![
            "INBOX".to_string(),
            "收件箱".to_string(),
            "Inbox".to_string(),
        ]);
        
        // 已发送的可能名称
        folder_mappings.insert("SENT".to_string(), vec![
            "SENT".to_string(),
            "Sent".to_string(),
            "已发送".to_string(),
            "Sent Messages".to_string(),
            "Sent Items".to_string(),
            "发件箱".to_string(),
            "OUTBOX".to_string(),
        ]);
        
        // 草稿的可能名称
        folder_mappings.insert("DRAFTS".to_string(), vec![
            "DRAFTS".to_string(),
            "Drafts".to_string(),
            "草稿".to_string(),
            "草稿箱".to_string(),
            "Draft".to_string(),
        ]);
        
        // 垃圾邮件的可能名称
        folder_mappings.insert("SPAM".to_string(), vec![
            "SPAM".to_string(),
            "Spam".to_string(),
            "Junk".to_string(),
            "JUNK".to_string(),
            "垃圾邮件".to_string(),
            "垃圾箱".to_string(),
            "Junk E-mail".to_string(),
            "Bulk Mail".to_string(),
        ]);
        
        // 垃圾箱的可能名称
        folder_mappings.insert("TRASH".to_string(), vec![
            "TRASH".to_string(),
            "Trash".to_string(),
            "Deleted".to_string(),
            "Deleted Items".to_string(),
            "Deleted Messages".to_string(),
            "已删除".to_string(),
            "回收站".to_string(),
            "垃圾桶".to_string(),
        ]);

        // 星标/收藏的可能名称
        folder_mappings.insert("STARRED".to_string(), vec![
            "STARRED".to_string(),
            "Starred".to_string(),
            "Flagged".to_string(),
            "收藏".to_string(),
            "收藏夹".to_string(),
            "收藏文件夹".to_string(),
            "星标".to_string(),
            "重要".to_string(),
            "&UXZO1mWHTvZZOQ-".to_string(), // QQ邮箱的收藏文件夹编码
        ]);
        
        Self {
            folder_mappings,
            account_folder_cache: Mutex::new(HashMap::new()),
        }
    }
    
    /// 获取服务器上的所有文件夹列表
    pub async fn list_folders(&self, account: &EmailAccount) -> Result<Vec<String>> {
        // 连接IMAP服务器
        let domain = &account.imap_server;
        let port = account.imap_port;

        // 创建TCP连接
        let tcp_stream = TcpStream::connect((domain.as_str(), port)).await
            .context("无法连接到IMAP服务器")?;

        // 创建TLS连接
        let tls_connector = TlsConnector::new();
        let tls_stream = tls_connector.connect(domain, tcp_stream).await
            .context("TLS握手失败")?;

        let client = Client::new(tls_stream);

        // 根据认证类型选择登录方式
        let mut session = if account.auth_type.as_deref() == Some("oauth2") {
            // OAuth2认证
            if let Some(access_token) = &account.access_token {
                tracing::debug!("使用OAuth2认证进行IMAP登录");
                crate::email_service::EmailService::oauth2_login(client, &account.username, access_token).await
                    .context("OAuth2 IMAP登录失败")?
            } else {
                return Err(anyhow::anyhow!("OAuth2账户缺少访问令牌"));
            }
        } else {
            // 传统用户名/密码认证
            tracing::debug!("使用用户名/密码认证进行IMAP登录");
            client.login(&account.username, &account.password).await
                .map_err(|e| anyhow::anyhow!("IMAP登录失败: {:?}", e.0))?
        };

        // 列出所有文件夹
        let folders_stream = session.list(None, Some("*")).await
            .context("获取文件夹列表失败")?;

        // 收集 Stream 为 Vec
        use futures::stream::StreamExt;
        let folders: Vec<_> = folders_stream.collect().await;

        let folder_names: Vec<String> = folders
            .into_iter()
            .filter_map(|folder_result| folder_result.ok())
            .map(|folder| folder.name().to_string())
            .collect();

        session.logout().await.context("IMAP登出失败")?;

        Ok(folder_names)
    }
    
    /// 将标准文件夹名称映射到服务器上的实际文件夹名称
    pub async fn map_folder(&self, account: &EmailAccount, standard_folder: &str) -> Result<String> {
        // 如果是INBOX，直接返回（INBOX是IMAP标准，所有服务器都支持）
        if standard_folder.to_uppercase() == "INBOX" {
            return Ok("INBOX".to_string());
        }

        // 如果是Gmail账户且使用OAuth2，直接使用Gmail API标准标签，不需要IMAP连接
        if account.imap_server.contains("gmail") && account.auth_type.as_deref() == Some("oauth2") {
            let gmail_folder = match standard_folder.to_uppercase().as_str() {
                "SENT" => "SENT",
                "DRAFTS" => "DRAFT", // Gmail使用DRAFT而不是DRAFTS
                "SPAM" => "SPAM",
                "TRASH" => "TRASH",
                "JUNK" => "SPAM", // 垃圾邮件的别名
                "DELETED" => "TRASH", // 已删除的别名
                _ => standard_folder, // 其他情况直接使用原名称
            };
            tracing::debug!("Gmail文件夹映射: {} -> {}", standard_folder, gmail_folder);
            return Ok(gmail_folder.to_string());
        }

        // 如果是Outlook账户且使用OAuth2，直接使用Graph API标准文件夹，不需要IMAP连接
        if (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) && account.auth_type.as_deref() == Some("oauth2") {
            let outlook_folder = match standard_folder.to_uppercase().as_str() {
                "SENT" => "SENT",
                "DRAFTS" => "DRAFTS",
                "SPAM" => "SPAM",
                "JUNK" => "SPAM", // 垃圾邮件的别名
                "TRASH" => "TRASH",
                "DELETED" => "TRASH", // 已删除的别名
                _ => standard_folder, // 其他情况直接使用原名称
            };
            tracing::debug!("Outlook文件夹映射: {} -> {}", standard_folder, outlook_folder);
            return Ok(outlook_folder.to_string());
        }

        // 检查缓存
        {
            let cache = self.account_folder_cache.lock().unwrap();
            if let Some(account_mappings) = cache.get(&account.id) {
                if let Some(mapped_folder) = account_mappings.get(&standard_folder.to_uppercase()) {
                    tracing::debug!("使用缓存的文件夹映射: {} -> {}", standard_folder, mapped_folder);
                    return Ok(mapped_folder.clone());
                }
            }
        }

        // 获取服务器上的文件夹列表
        let server_folders = self.list_folders(account).await?;
        
        // 查找映射
        let mut found_folder = None;
        if let Some(possible_names) = self.folder_mappings.get(&standard_folder.to_uppercase()) {
            for possible_name in possible_names {
                // 检查服务器是否有这个文件夹（不区分大小写）
                for server_folder in &server_folders {
                    if server_folder.to_lowercase() == possible_name.to_lowercase() {
                        found_folder = Some(server_folder.clone());
                        break;
                    }
                }
                if found_folder.is_some() { break; }
            }
        }

        // 如果没有找到映射，尝试直接使用原名称
        if found_folder.is_none() {
            for server_folder in &server_folders {
                if server_folder.to_lowercase() == standard_folder.to_lowercase() {
                    found_folder = Some(server_folder.clone());
                    break;
                }
            }
        }

        if let Some(folder) = found_folder {
            // 更新缓存
            {
                let mut cache = self.account_folder_cache.lock().unwrap();
                let account_mappings = cache.entry(account.id.clone()).or_insert_with(HashMap::new);
                account_mappings.insert(standard_folder.to_uppercase(), folder.clone());
            }
            tracing::debug!("缓存文件夹映射: {} -> {}", standard_folder, folder);
            Ok(folder)
        } else {
            // 如果还是没找到，返回错误并提供可用的文件夹列表
            Err(anyhow::anyhow!(
                "找不到文件夹 '{}'\n可用的文件夹: {}",
                standard_folder,
                server_folders.join(", ")
            ))
        }
    }
    
    /// 获取账户的文件夹映射缓存
    pub async fn get_folder_mappings(&self, account: &EmailAccount) -> Result<HashMap<String, String>> {
        let mut mappings = HashMap::new();

        // 如果是Gmail账户且使用OAuth2，直接返回Gmail标准文件夹映射
        if account.imap_server.contains("gmail") && account.auth_type.as_deref() == Some("oauth2") {
            mappings.insert("INBOX".to_string(), "INBOX".to_string());
            mappings.insert("SENT".to_string(), "SENT".to_string());
            mappings.insert("DRAFTS".to_string(), "DRAFT".to_string());
            mappings.insert("SPAM".to_string(), "SPAM".to_string());
            mappings.insert("TRASH".to_string(), "TRASH".to_string());
            tracing::debug!("返回Gmail标准文件夹映射");
            return Ok(mappings);
        }

        // 如果是Outlook账户且使用OAuth2，直接返回Outlook标准文件夹映射
        if (account.imap_server.contains("outlook") || account.imap_server.contains("office365")) && account.auth_type.as_deref() == Some("oauth2") {
            mappings.insert("INBOX".to_string(), "INBOX".to_string());
            mappings.insert("SENT".to_string(), "SENT".to_string());
            mappings.insert("DRAFTS".to_string(), "DRAFTS".to_string());
            mappings.insert("SPAM".to_string(), "SPAM".to_string());
            mappings.insert("TRASH".to_string(), "TRASH".to_string());
            tracing::debug!("返回Outlook标准文件夹映射");
            return Ok(mappings);
        }

        // 对于其他账户，使用IMAP连接获取文件夹列表
        let server_folders = self.list_folders(account).await?;
        
        // 为每个标准文件夹找到对应的服务器文件夹
        for (standard_name, possible_names) in &self.folder_mappings {
            let mut found = false;
            
            for possible_name in possible_names {
                for server_folder in &server_folders {
                    if server_folder.to_lowercase() == possible_name.to_lowercase() {
                        mappings.insert(standard_name.clone(), server_folder.clone());
                        found = true;
                        break;
                    }
                }
                if found { break; }
            }
            
            // 如果没找到，但有同名的文件夹，也添加进去
            if !found {
                for server_folder in &server_folders {
                    if server_folder.to_lowercase() == standard_name.to_lowercase() {
                        mappings.insert(standard_name.clone(), server_folder.clone());
                        break;
                    }
                }
            }
        }
        
        // INBOX总是存在
        mappings.insert("INBOX".to_string(), "INBOX".to_string());
        
        Ok(mappings)
    }
    
    /// 检查文件夹是否存在
    pub async fn folder_exists(&self, account: &EmailAccount, folder_name: &str) -> Result<bool> {
        let server_folders = self.list_folders(account).await?;
        Ok(server_folders.iter().any(|f| f.to_lowercase() == folder_name.to_lowercase()))
    }
}
