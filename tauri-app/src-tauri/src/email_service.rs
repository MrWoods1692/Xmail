use anyhow::{Result, Context};
use async_imap::Client;
use async_imap::types::Flag;
use async_imap::Session;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::database::models::{EmailAccount, EmailMessage};
use mail_parser::MessageParser;
use std::collections::HashMap;
use async_std::net::TcpStream;
use async_native_tls::{TlsConnector, TlsStream};
use futures::StreamExt;
use sqlx::Row;


pub struct EmailService;

impl EmailService {
    /// 查找已发送文件夹（使用与SMTP保存相同的逻辑）
    pub async fn find_sent_folder(account: &EmailAccount) -> Result<String> {
        use async_std::net::TcpStream;
        use async_native_tls::TlsConnector;
        use futures::stream::StreamExt;

        // 连接到IMAP服务器
        let domain = &account.imap_server;
        let port = account.imap_port;

        let tcp_stream = TcpStream::connect((domain.as_str(), port)).await
            .context("连接IMAP服务器失败")?;

        let tls = TlsConnector::new();
        let tls_stream = tls.connect(domain, tcp_stream).await
            .context("TLS连接失败")?;

        let client = async_imap::Client::new(tls_stream);
        let mut session = if account.auth_type.as_ref().map(|s| s.as_str()) == Some("oauth2") {
            Self::oauth2_login_with_retry(domain, port, &account.email, &account.access_token.as_ref().unwrap_or(&String::new()), 1).await?
        } else {
            client.login(&account.email, &account.password).await
                .map_err(|e| anyhow::anyhow!("IMAP登录失败: {:?}", e))?
        };

        // 列出所有文件夹
        let mut folders_stream = session.list(None, Some("*")).await
            .context("列出文件夹失败")?;

        let mut sent_folder = None;
        while let Some(folder_result) = folders_stream.next().await {
            match folder_result {
                Ok(folder) => {
                    let folder_name = folder.name().to_lowercase();
                    tracing::debug!("检查文件夹: {}", folder.name());

                    if folder_name.contains("sent") ||
                       folder_name.contains("已发送") ||
                       folder_name.contains("sentitems") ||
                       folder_name.contains("发件箱") ||
                       folder_name == "sent items" {
                        sent_folder = Some(folder.name().to_string());
                        tracing::info!("找到已发送文件夹: {}", folder.name());
                        break;
                    }
                },
                Err(e) => {
                    tracing::warn!("读取文件夹信息失败: {}", e);
                }
            }
        }

        // 显式释放stream，然后logout
        drop(folders_stream);
        session.logout().await.ok();

        sent_folder.ok_or_else(|| anyhow::anyhow!("未找到已发送文件夹"))
    }

    /// OAuth2 IMAP登录（带重试机制）
    pub async fn oauth2_login_with_retry(
        domain: &str,
        port: u16,
        username: &str,
        access_token: &str,
        max_retries: u32
    ) -> Result<Session<TlsStream<TcpStream>>> {
        let mut last_error = None;

        for attempt in 1..=max_retries {
            tracing::debug!("OAuth2 IMAP登录尝试 {}/{}", attempt, max_retries);

            // 创建新的连接
            let tcp_stream = match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                TcpStream::connect((domain, port))
            ).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => {
                    last_error = Some(anyhow::anyhow!("连接失败: {}", e));
                    if attempt < max_retries {
                        tracing::warn!("连接失败，{}秒后重试 ({}/{}): {}", attempt * 2, attempt, max_retries, e);
                        tokio::time::sleep(std::time::Duration::from_secs(attempt as u64 * 2)).await;
                        continue;
                    }
                    break;
                },
                Err(_) => {
                    last_error = Some(anyhow::anyhow!("连接超时"));
                    if attempt < max_retries {
                        tracing::warn!("连接超时，{}秒后重试 ({}/{})", attempt * 2, attempt, max_retries);
                        tokio::time::sleep(std::time::Duration::from_secs(attempt as u64 * 2)).await;
                        continue;
                    }
                    break;
                }
            };

            // 创建TLS连接
            let tls_connector = TlsConnector::new();
            let tls_stream = match tls_connector.connect(domain, tcp_stream).await {
                Ok(stream) => stream,
                Err(e) => {
                    last_error = Some(anyhow::anyhow!("TLS握手失败: {}", e));
                    if attempt < max_retries {
                        tracing::warn!("TLS握手失败，{}秒后重试 ({}/{}): {}", attempt * 2, attempt, max_retries, e);
                        tokio::time::sleep(std::time::Duration::from_secs(attempt as u64 * 2)).await;
                        continue;
                    }
                    break;
                }
            };

            let client = Client::new(tls_stream);

            // 尝试OAuth2认证
            match Self::oauth2_login(client, username, access_token).await {
                Ok(session) => {
                    tracing::info!("OAuth2 IMAP登录成功 (尝试 {}/{})", attempt, max_retries);
                    return Ok(session);
                },
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        tracing::warn!("OAuth2认证失败，{}秒后重试 ({}/{}): {}", attempt * 2, attempt, max_retries, last_error.as_ref().unwrap());
                        tokio::time::sleep(std::time::Duration::from_secs(attempt as u64 * 2)).await;
                        continue;
                    }
                    break;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("OAuth2登录失败，已达到最大重试次数")))
    }

    /// QQ邮箱连接健康检查
    pub async fn qq_connection_health_check(account: &EmailAccount) -> Result<bool> {
        if !account.imap_server.contains("qq.com") {
            return Ok(true); // 非QQ邮箱直接返回健康
        }

        tracing::debug!("执行QQ邮箱连接健康检查: {}", account.email);

        // 尝试快速连接测试
        let domain = &account.imap_server;
        let port = account.imap_port;

        let tcp_stream = match tokio::time::timeout(
            std::time::Duration::from_secs(10), // 健康检查使用较短超时
            TcpStream::connect((domain.as_str(), port))
        ).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(_)) | Err(_) => {
                tracing::warn!("QQ邮箱连接健康检查失败: 无法建立TCP连接");
                return Ok(false);
            }
        };

        // 简单的TLS握手测试
        let tls_connector = TlsConnector::new();
        match tls_connector.connect(domain, tcp_stream).await {
            Ok(_) => {
                tracing::debug!("QQ邮箱连接健康检查通过");
                Ok(true)
            },
            Err(_) => {
                tracing::warn!("QQ邮箱连接健康检查失败: TLS握手失败");
                Ok(false)
            }
        }
    }

    /// OAuth2 IMAP登录
    pub async fn oauth2_login(
        client: Client<TlsStream<TcpStream>>,
        username: &str,
        access_token: &str
    ) -> Result<Session<TlsStream<TcpStream>>> {
        // 构造OAuth2 SASL XOAUTH2字符串
        let auth_string = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            username, access_token
        );

        // 使用base64编码
        use base64::{Engine as _, engine::general_purpose};
        let auth_string_b64 = general_purpose::STANDARD.encode(&auth_string);

        tracing::debug!("OAuth2认证字符串长度: {}", auth_string_b64.len());
        tracing::debug!("OAuth2认证字符串: {}", auth_string_b64);
        tracing::debug!("原始认证字符串: {}", auth_string);

        // 创建自定义的OAuth2认证器
        struct OAuth2Authenticator {
            auth_string: String,
        }

        impl async_imap::Authenticator for OAuth2Authenticator {
            type Response = String;

            fn process(&mut self, _challenge: &[u8]) -> Self::Response {
                self.auth_string.clone()
            }
        }

        let authenticator = OAuth2Authenticator {
            auth_string: auth_string_b64.clone(),
        };

        // 使用AUTHENTICATE命令进行OAuth2认证
        let session = client.authenticate("XOAUTH2", authenticator).await
            .map_err(|e| {
                tracing::error!("OAuth2 IMAP认证详细错误: {:?}", e);
                tracing::error!("认证字符串长度: {}", auth_string_b64.len());
                tracing::error!("认证字符串前50字符: {}", &auth_string_b64[..std::cmp::min(50, auth_string_b64.len())]);
                anyhow::anyhow!("OAuth2 IMAP认证失败: {:?}", e.0)
            })?;

        Ok(session)
    }
    /// 测试邮箱账户连接
    pub async fn test_connection(account: &EmailAccount) -> Result<bool> {
        tracing::debug!("开始测试IMAP连接: {}:{}", account.imap_server, account.imap_port);

        // 使用 async-imap 进行异步连接
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
                // Gmail使用邮箱地址而不是用户名
                let oauth_username = if account.imap_server.contains("gmail") {
                    &account.email
                } else {
                    &account.username
                };
                Self::oauth2_login(client, oauth_username, access_token).await
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

        // 测试选择收件箱
        session.select("INBOX").await
            .context("无法选择INBOX文件夹")?;

        // 登出
        session.logout().await
            .context("IMAP登出失败")?;

        tracing::debug!("IMAP连接测试成功");
        Ok(true)
    }
    
    /// 获取邮件列表
    pub async fn fetch_messages(account: &EmailAccount, folder: &str, limit: Option<i64>) -> Result<Vec<EmailMessage>> {
        Self::fetch_messages_with_db(account, folder, limit, None).await
    }

    /// 获取邮件列表（带数据库连接，用于令牌刷新）
    pub async fn fetch_messages_with_db(account: &EmailAccount, folder: &str, limit: Option<i64>, database: Option<&sqlx::MySqlPool>) -> Result<Vec<EmailMessage>> {
        // 使用 async-imap 进行异步连接
        let domain = &account.imap_server;
        let port = account.imap_port;

        tracing::debug!("尝试连接IMAP服务器: {}:{}", domain, port);

        // 根据邮箱提供商调整连接超时时间
        let timeout_duration = if domain.contains("qq.com") {
            // QQ邮箱使用更长的超时时间
            std::time::Duration::from_secs(45)
        } else {
            std::time::Duration::from_secs(30)
        };

        // 创建TCP连接，增加超时设置
        let tcp_stream = tokio::time::timeout(
            timeout_duration,
            TcpStream::connect((domain.as_str(), port))
        ).await
            .context("连接IMAP服务器超时")?
            .context(format!("无法连接到IMAP服务器 {}:{}", domain, port))?;

        // 创建TLS连接
        let tls_connector = TlsConnector::new();
        let tls_stream = tls_connector.connect(domain, tcp_stream).await
            .context("TLS握手失败")?;

        let client = Client::new(tls_stream);

        tracing::debug!("尝试IMAP登录，用户名: {}", account.username);

        // 根据认证类型选择登录方式
        let mut session = if account.auth_type.as_deref() == Some("oauth2") {
            // OAuth2认证
            if let Some(access_token) = &account.access_token {
                tracing::debug!("使用OAuth2认证进行IMAP登录");
                tracing::debug!("OAuth2访问令牌长度: {}", access_token.len());
                tracing::debug!("OAuth2访问令牌前缀: {}", &access_token[..std::cmp::min(20, access_token.len())]);

                // 尝试使用当前令牌登录（Gmail使用邮箱地址而不是用户名）
                let oauth_username = if account.imap_server.contains("gmail") {
                    &account.email
                } else {
                    &account.username
                };

                // 根据邮箱提供商调整重试次数
                let max_retries = if domain.contains("qq.com") {
                    // QQ邮箱使用更多重试次数
                    5
                } else {
                    3
                };

                // 首先尝试使用带重试的OAuth2登录
                match Self::oauth2_login_with_retry(&domain, port, oauth_username, access_token, max_retries).await {
                    Ok(session) => session,
                    Err(e) => {
                        tracing::warn!("OAuth2登录失败，可能令牌已过期: {}", e);

                        // 如果有刷新令牌，尝试刷新访问令牌
                        if let Some(refresh_token) = &account.refresh_token {
                            tracing::info!("尝试刷新OAuth2访问令牌");

                            // 根据账户类型选择正确的OAuth2配置和刷新方法
                            let refresh_result = if account.imap_server.contains("outlook") || account.imap_server.contains("office365") {
                                // Outlook账户
                                let oauth_config = crate::oauth2::get_outlook_oauth2_config();
                                let oauth_client = crate::oauth2::OAuth2Client::new(oauth_config);
                                oauth_client.refresh_outlook_token(refresh_token).await
                            } else {
                                // Gmail账户
                                let oauth_config = crate::oauth2::get_gmail_oauth2_config();
                                let oauth_client = crate::oauth2::OAuth2Client::new(oauth_config);
                                oauth_client.refresh_token(refresh_token).await
                            };

                            match refresh_result {
                                Ok(new_token) => {
                                    tracing::info!("OAuth2令牌刷新成功");

                                    // 更新数据库中的访问令牌
                                    if let Some(db) = database {
                                        if let Err(e) = crate::update_oauth2_token_internal(db, &account.id, &new_token.access_token).await {
                                            tracing::error!("更新数据库中的OAuth2令牌失败: {}", e);
                                        }
                                    }

                                    // 使用刷新后的令牌重新登录（带重试机制）
                                    Self::oauth2_login_with_retry(&domain, port, oauth_username, &new_token.access_token, 3).await
                                        .context("使用刷新后的令牌登录失败")?
                                },
                                Err(refresh_err) => {
                                    tracing::error!("OAuth2令牌刷新失败: {}", refresh_err);
                                    return Err(anyhow::anyhow!("OAuth2认证失败，令牌已过期且无法刷新: {}", refresh_err));
                                }
                            }
                        } else {
                            return Err(anyhow::anyhow!("OAuth2认证失败且无刷新令牌: {}", e));
                        }
                    }
                }
            } else {
                return Err(anyhow::anyhow!("OAuth2账户缺少访问令牌"));
            }
        } else {
            // 传统用户名/密码认证
            tracing::debug!("使用用户名/密码认证进行IMAP登录");
            client.login(&account.username, &account.password).await
                .map_err(|e| anyhow::anyhow!("IMAP登录失败: {:?}", e.0))?
        };

        // 选择文件夹
        tracing::debug!("选择邮件文件夹: {}", folder);
        let mailbox = session.select(folder).await
            .context(format!("无法选择文件夹: {}", folder))?;

        let total_messages = mailbox.exists;
        tracing::debug!("文件夹 {} 服务器报告 {} 封邮件（包含重复）", folder, total_messages);

        // 注意：服务器报告的数量包含重复邮件，实际有用邮件数量会更少
        tracing::debug!("IMAP邮箱状态 - EXISTS: {}, RECENT: {}, UNSEEN: {:?}",
            mailbox.exists,
            mailbox.recent,
            mailbox.unseen
        );
        if total_messages == 0 {
            tracing::debug!("文件夹为空，返回空列表");
            session.logout().await.context("IMAP登出失败")?;
            return Ok(vec![]);
        }

        // 极简获取策略：只获取真正需要的邮件数量
        let actual_limit = if let Some(limit_val) = limit {
            limit_val as u32
        } else {
            // 初始加载：只获取最新的120封邮件（经验值：去重后约100封有效邮件）
            120
        };

        let (start, end) = {
            let start = if total_messages > actual_limit {
                total_messages - actual_limit + 1
            } else {
                1
            };
            (start, total_messages)
        };

        tracing::debug!("极简获取邮件范围: {}:{} (共{}封，只获取必要邮件)", start, end, end - start + 1);

        // 分批获取邮件，避免一次获取太多导致超时或失败
        let batch_size = 10; // 每批最多10封邮件
        let mut email_messages = Vec::new();

        let mut current_start = start;
        while current_start <= end {
            let current_end = std::cmp::min(current_start + batch_size - 1, end);
            tracing::debug!("获取邮件批次: {}:{}", current_start, current_end);

            // 分别获取FLAGS和RFC822，确保兼容性
            tracing::debug!("获取邮件FLAGS，范围: {}:{}", current_start, current_end);

            // 首先获取FLAGS
            let flags_map: HashMap<u32, bool> = {
                let flags_result = session.fetch(format!("{}:{}", current_start, current_end), "FLAGS").await;
                match flags_result {
                    Ok(flag_messages_stream) => {
                        tracing::debug!("成功获取FLAGS");
                        // 收集 Stream 为 Vec
                        use futures::stream::StreamExt;
                        let flag_messages: Vec<_> = flag_messages_stream.collect().await;

                        flag_messages.into_iter()
                            .filter_map(|msg_result| msg_result.ok())
                            .map(|msg| {
                                let seq = msg.message;
                                let is_read = msg.flags().any(|flag| {
                                    matches!(flag, Flag::Seen)
                                });
                                (seq, is_read)
                            })
                            .collect()
                    },
                    Err(e) => {
                        tracing::warn!("获取FLAGS失败: {}，所有邮件将标记为未读", e);
                        HashMap::new()
                    }
                }
            };

            // 使用FETCH获取正确的序列号到UID映射
            tracing::debug!("获取邮件UID映射，范围: {}:{}", current_start, current_end);
            let uid_fetch_stream = session.fetch(format!("{}:{}", current_start, current_end), "UID").await
                .context(format!("获取邮件UID映射失败，范围: {}:{}", current_start, current_end))?;

            let uid_fetch_messages: Vec<_> = uid_fetch_stream.collect().await;

            // 构建正确的序列号到UID映射
            let uid_map: HashMap<u32, u32> = uid_fetch_messages.into_iter()
                .filter_map(|message_result| message_result.ok())
                .filter_map(|message| {
                    // 从FETCH响应中提取UID
                    let seq = message.message;
                    if let Some(uid_value) = message.uid {
                        Some((seq, uid_value))
                    } else {
                        tracing::warn!("邮件序列号{}没有UID信息", seq);
                        None
                    }
                })
                .collect();

            tracing::debug!("构建UID映射，共{}个条目: {:?}", uid_map.len(), uid_map);

            // 然后获取RFC822
            tracing::debug!("获取邮件内容，范围: {}:{}", current_start, current_end);
            let messages_stream = session.fetch(format!("{}:{}", current_start, current_end), "RFC822").await
                .context(format!("获取邮件失败，范围: {}:{}", current_start, current_end))?;

            // 收集 Stream 为 Vec
            use futures::stream::StreamExt;
            let messages: Vec<_> = messages_stream.collect().await;

            // 将messages转换为拥有所有权的数据结构，避免生命周期问题
            let message_data: Vec<(Vec<u8>, bool, Option<u32>)> = messages.into_iter()
                .filter_map(|message_result| message_result.ok())
                .filter_map(|message| {
                    if let Some(body) = message.body() {
                        // 从FLAGS映射中获取已读状态
                        let seq = message.message;
                        let is_read = flags_map.get(&seq).copied().unwrap_or(false);
                        // 从UID映射中获取UID
                        let uid = uid_map.get(&seq).copied();

                        Some((body.to_vec(), is_read, uid))
                    } else {
                        tracing::warn!("邮件消息没有body内容");
                        None
                    }
                })
                .collect();

            // 现在处理拥有所有权的数据
            for (i, (body, is_read, uid)) in message_data.iter().enumerate() {
                match Self::parse_email_message_with_account(body, &account.id, folder, *is_read, *uid, None) {
                    Ok(email_msg) => {
                        tracing::debug!("成功解析邮件 {} (UID: {:?}): {}", i + 1, uid, email_msg.subject);
                        email_messages.push(email_msg);
                    },
                    Err(e) => {
                        tracing::error!("解析邮件失败 (第{}封): {}", i + 1, e);
                        continue;
                    }
                }
            }

            current_start = current_end + 1;
        }

        session.logout().await.context("IMAP登出失败")?;

        // 按时间倒序排列（最新的在前面）
        email_messages.reverse();

        Ok(email_messages)
    }

    /// 快速查找邮件（真正的批量处理）
    async fn find_email_by_message_id_fast(session: &mut Session<TlsStream<TcpStream>>, message_id: &str) -> Option<u32> {
        if let Ok(seq_nums) = session.search("ALL").await {
            if !seq_nums.is_empty() {
                let mut seq_vec: Vec<u32> = seq_nums.iter().copied().collect();
                seq_vec.sort();

                tracing::info!("开始批量搜索所有{}封邮件", seq_vec.len());

                // 真正的批量处理：一次获取所有邮件的RFC822
                let all_seqs = seq_vec.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(",");

                if let Ok(mut fetch_stream) = session.fetch(all_seqs, "RFC822").await {
                    use futures::stream::StreamExt;
                    while let Some(Ok(fetch)) = fetch_stream.next().await {
                        if let Some(body) = fetch.body() {
                            let header_text = String::from_utf8_lossy(body);
                            // 只检查前1000个字符（邮件头部分）
                            let header_part = if header_text.len() > 1000 {
                                &header_text[..1000]
                            } else {
                                &header_text
                            };

                            for line in header_part.lines() {
                                if line.to_lowercase().starts_with("message-id:") && line.contains(message_id) {
                                    let seq_num = fetch.message;
                                    tracing::info!("找到邮件，序列号: {}", seq_num);
                                    return Some(seq_num);
                                }
                            }
                        }
                    }
                }

                tracing::warn!("批量搜索了所有{}封邮件，未找到匹配的Message-ID", seq_vec.len());
            }
        }
        None
    }

    /// 通过Message-ID查找邮件，返回序列号
    async fn find_email_by_message_id(session: &mut Session<TlsStream<TcpStream>>, message_id: &str) -> Option<u32> {
        tracing::info!("开始查找邮件，目标Message-ID: {}", message_id);

        // 检查是否是UUID格式（我们生成的本地ID）
        if message_id.len() == 36 && message_id.chars().filter(|&c| c == '-').count() == 4 {
            tracing::warn!("传入的是UUID格式的本地ID，无法在IMAP服务器上查找: {}", message_id);
            return None;
        }

        // 尝试多种Message-ID搜索格式（QQ邮箱兼容性）
        let search_patterns = vec![
            format!("HEADER Message-ID <{}>", message_id),      // 尖括号格式，无引号
            format!("HEADER Message-ID \"<{}>\"", message_id),  // 标准格式
            format!("HEADER Message-ID \"{}\"", message_id),    // 无尖括号格式
            format!("HEADER Message-ID {}", message_id),        // 无引号格式
            format!("HEADER Message-ID <{}>", format!("{}", message_id)), // 确保尖括号格式
        ];

        for (i, search_pattern) in search_patterns.iter().enumerate() {
            tracing::info!("搜索邮件Message-ID (尝试 {}): {}", i + 1, search_pattern);

            // 尝试UID搜索
            match session.uid_search(search_pattern).await {
                Ok(uids) => {
                    if !uids.is_empty() {
                        let uid = *uids.iter().next().unwrap();
                        tracing::info!("通过UID搜索找到邮件，UID: {} (格式 {})", uid, i + 1);
                        return Some(uid);
                    }
                },
                Err(e) => {
                    tracing::debug!("UID搜索失败 (格式 {}): {}", i + 1, e);
                }
            }

            // 尝试普通搜索
            match session.search(search_pattern).await {
                Ok(seq_nums) => {
                    if !seq_nums.is_empty() {
                        let seq = *seq_nums.iter().next().unwrap();
                        tracing::info!("通过普通搜索找到邮件，序列号: {} (格式 {})", seq, i + 1);
                        return Some(seq);
                    }
                },
                Err(e) => {
                    tracing::debug!("普通搜索失败 (格式 {}): {}", i + 1, e);
                }
            }
        }

        // 如果所有Message-ID搜索格式都失败，直接返回None
        tracing::warn!("所有Message-ID搜索格式都失败，无法找到邮件: {}", message_id);
        None
    }

    /// 通过邮件元数据（主题+发件人+日期）查找邮件
    async fn find_email_by_metadata(
        session: &mut Session<TlsStream<TcpStream>>,
        subject: &str,
        from: &str,
        date: &str
    ) -> Option<u32> {
        tracing::info!("通过元数据查找邮件: 主题={}, 发件人={}, 日期={}", subject, from, date);

        // 尝试通过主题搜索
        let subject_search = format!("SUBJECT \"{}\"", subject.replace("\"", "\\\""));
        match session.search(&subject_search).await {
            Ok(seq_nums) => {
                if !seq_nums.is_empty() {
                    // 如果找到多个结果，返回第一个
                    let seq = *seq_nums.iter().next().unwrap();
                    tracing::info!("通过主题搜索找到邮件，序列号: {}", seq);
                    return Some(seq);
                }
            },
            Err(e) => {
                tracing::debug!("主题搜索失败: {}", e);
            }
        }

        // 尝试通过发件人搜索
        let from_search = format!("FROM \"{}\"", from.replace("\"", "\\\""));
        match session.search(&from_search).await {
            Ok(seq_nums) => {
                if !seq_nums.is_empty() {
                    // 如果找到多个结果，返回第一个
                    let seq = *seq_nums.iter().next().unwrap();
                    tracing::info!("通过发件人搜索找到邮件，序列号: {}", seq);
                    return Some(seq);
                }
            },
            Err(e) => {
                tracing::debug!("发件人搜索失败: {}", e);
            }
        }

        tracing::warn!("通过元数据也无法找到邮件");
        None
    }



    /// 删除邮件（简化版本）
    pub async fn delete_message_simple(
        account: &EmailAccount,
        message_id: &str,
        current_folder: &str,
        email_database: &crate::EmailDatabase
    ) -> Result<bool> {
        // 使用 async-imap 进行异步连接
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
                // Gmail使用邮箱地址而不是用户名
                let oauth_username = if account.imap_server.contains("gmail") {
                    &account.email
                } else {
                    &account.username
                };
                Self::oauth2_login(client, oauth_username, access_token).await
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

        // 选择当前文件夹
        let mailbox = session.select(current_folder).await
            .context(format!("无法选择文件夹: {}", current_folder))?;

        let total_messages = mailbox.exists;
        if total_messages == 0 {
            tracing::warn!("文件夹为空，无邮件可删除");
            session.logout().await.context("IMAP登出失败")?;
            return Ok(false);
        }

        // QQ邮箱不支持Message-ID搜索，使用数据库信息进行搜索
        tracing::info!("QQ邮箱删除邮件，获取数据库信息进行搜索: {}", message_id);
        tracing::info!("当前账户信息 - IMAP服务器: {}, 邮箱: {}, 文件夹: {}",
                      account.imap_server, account.email, current_folder);

        // 从数据库获取保存的IMAP UID（Android Mail方式）
        let target_uid = match sqlx::query("SELECT imap_uid FROM emails WHERE account_id = ? AND message_id = ?")
            .bind(&account.id)
            .bind(message_id)
            .fetch_optional(email_database.pool())
            .await
        {
            Ok(Some(row)) => {
                match row.try_get::<Option<i64>, _>("imap_uid") {
                    Ok(Some(uid)) => {
                        tracing::info!("✅ 从数据库获取到IMAP UID: {}", uid);
                        uid as u32
                    },
                    Ok(None) => {
                        tracing::error!("邮件记录中没有IMAP UID，无法删除: {}", message_id);
                        session.logout().await.context("IMAP登出失败")?;
                        return Ok(false);
                    },
                    Err(e) => {
                        tracing::error!("读取IMAP UID失败: {}", e);
                        session.logout().await.context("IMAP登出失败")?;
                        return Ok(false);
                    }
                }
            },
            Ok(None) => {
                tracing::error!("未找到要删除的邮件记录: {}", message_id);
                session.logout().await.context("IMAP登出失败")?;
                return Ok(false);
            },
            Err(e) => {
                tracing::error!("查询邮件记录失败: {}", e);
                session.logout().await.context("IMAP登出失败")?;
                return Ok(false);
            }
        };

        tracing::info!("找到邮件IMAP UID: {}", target_uid);

        // 添加调试信息
        tracing::info!("账户IMAP服务器: {}", account.imap_server);
        tracing::info!("邮件Message-ID: {}", message_id);

        let uid_str = target_uid.to_string();

        // 根据当前文件夹决定删除方式
        if current_folder == "Deleted Messages" || current_folder == "TRASH" {
            // 在已删除文件夹中：永久删除
            tracing::info!("在已删除文件夹中删除邮件，UID: {}，执行永久删除", target_uid);

            let _store_result: Vec<_> = session.uid_store(&uid_str, "+FLAGS (\\Deleted)").await
                .context("标记邮件删除失败")?
                .collect().await;

            let _expunge_result: Vec<_> = session.expunge().await
                .context("EXPUNGE操作失败")?
                .collect().await;

            tracing::info!("✅ 邮件已永久删除: {}", message_id);
        } else {
            // 在其他文件夹中：移动到已删除文件夹
            tracing::info!("删除邮件，UID: {}，移动到Deleted Messages文件夹", target_uid);

            let move_result = session.uid_mv(&uid_str, "Deleted Messages").await;
            match move_result {
                Ok(_) => {
                    tracing::info!("✅ 邮件已移动到Deleted Messages文件夹: {}", message_id);
                },
                Err(e) => {
                    tracing::warn!("移动到Deleted Messages失败: {}，尝试标准删除", e);

                    // 备用方案：标准删除
                    let _store_result: Vec<_> = session.uid_store(&uid_str, "+FLAGS (\\Deleted)").await
                        .context("标记邮件删除失败")?
                        .collect().await;

                    let _expunge_result: Vec<_> = session.expunge().await
                        .context("EXPUNGE操作失败")?
                        .collect().await;

                    tracing::info!("✅ 使用标准删除方式完成: {}", message_id);
                }
            }
        }

        // 登出IMAP会话
        session.logout().await.context("IMAP登出失败")?;

        // 从数据库删除邮件记录（Android Mail方式：不验证服务器）
        match sqlx::query("DELETE FROM emails WHERE account_id = ? AND message_id = ?")
            .bind(&account.id)
            .bind(message_id)
            .execute(email_database.pool())
            .await
        {
            Ok(_) => {
                tracing::info!("✅ 已从数据库删除邮件记录（Android Mail方式）: {}", message_id);
                Ok(true)
            },
            Err(e) => {
                tracing::error!("从数据库删除邮件记录失败: {}", e);
                Err(anyhow::anyhow!("数据库删除失败: {}", e))
            }
        }
    }

    /// 解析邮件消息
    fn parse_email_message_with_account(raw_email: &[u8], account_id: &str, folder: &str, is_read: bool, imap_uid: Option<u32>, _account: Option<&EmailAccount>) -> Result<EmailMessage> {
        // 使用mail-parser库解析邮件
        let message = MessageParser::default()
            .parse(raw_email)
            .ok_or_else(|| anyhow::anyhow!("无法解析邮件"))?;

        // 提取主题
        let subject = message.subject()
            .unwrap_or("(无主题)")
            .to_string();

        // 提取发件人
        let from = message.from()
            .and_then(|addr| addr.first())
            .map(|addr| {
                if let Some(name) = &addr.name {
                    format!("{} <{}>", name, addr.address.as_ref().map(|s| s.as_ref()).unwrap_or(""))
                } else {
                    addr.address.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "未知发件人".to_string())
                }
            })
            .unwrap_or_else(|| {
                // 调试：打印邮件头信息
                tracing::info!("邮件没有From字段 - 文件夹: {}, 主题: {}", folder, subject);
                let from_headers: Vec<_> = message.header_values("From").collect();
                if let Some(first_from) = from_headers.first() {
                    let from_str = format!("{:?}", first_from);
                    tracing::info!("原始From头: {}", from_str);
                    from_str
                } else {
                    tracing::info!("邮件完全没有From头");
                    "未知发件人".to_string()
                }
            });

        // 提取收件人
        let recipients = message.to()
            .iter()
            .flat_map(|addrs| addrs.iter())
            .filter_map(|addr| addr.address.as_ref().map(|s| s.to_string()))
            .collect::<Vec<String>>();
        let recipients_json = serde_json::to_string(&recipients)
            .unwrap_or_else(|_| "[]".to_string());

        // 提取抄送
        let cc = if let Some(cc_addrs) = message.cc() {
            let cc_list: Vec<String> = cc_addrs.iter()
                .filter_map(|addr| addr.address.as_ref().map(|s| s.to_string()))
                .collect();
            Some(serde_json::to_string(&cc_list).unwrap_or_else(|_| "[]".to_string()))
        } else {
            None
        };

        // 提取邮件正文
        let body_text = message.body_text(0)
            .map(|text| Self::clean_text_content(&text))
            .or_else(|| {
                // 如果没有纯文本，尝试从HTML中提取
                message.body_html(0).map(|html| {
                    Self::html_to_text(&html)
                })
            });

        let body_html = message.body_html(0)
            .map(|html| html.to_string());

        // 提取日期
        let received_at = message.date()
            .map(|date| DateTime::from_timestamp(date.to_timestamp(), 0))
            .flatten()
            .unwrap_or_else(|| Utc::now());

        // 提取消息ID - 如果没有Message-ID，使用主题+发件人+时间的哈希作为稳定标识符
        let message_id = if let Some(msg_id) = message.message_id() {
            msg_id.to_string()
        } else {
            // 生成稳定的标识符：基于主题+发件人+接收时间的哈希
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            subject.hash(&mut hasher);
            from.hash(&mut hasher);
            received_at.timestamp().hash(&mut hasher);
            // 添加邮件内容和IMAP UID来增加唯一性
            body_text.hash(&mut hasher);
            if let Some(uid) = imap_uid {
                uid.hash(&mut hasher);
            }
            // 添加随机数确保唯一性
            uuid::Uuid::new_v4().to_string().hash(&mut hasher);

            format!("generated-{:x}", hasher.finish())
        };

        let now = Utc::now();

        Ok(EmailMessage {
            id: Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            message_id,
            subject,
            sender: from,
            recipients: recipients_json,
            cc,
            bcc: None, // BCC通常在接收的邮件中不可见
            body_text,
            body_html,
            folder: folder.to_string(),
            is_read,
            is_starred: false,
            is_deleted: false,
            received_at,
            created_at: now,
            updated_at: now,
            imap_uid: imap_uid.map(|uid| uid as i64),
        })
    }



    /// 清理文本内容，移除格式标记
    fn clean_text_content(text: &str) -> String {
        text
            // 移除Markdown格式标记
            .replace("**", "")  // 粗体
            .replace("*", "")   // 斜体或强调
            .replace("__", "")  // 下划线粗体
            .replace("_", "")   // 下划线斜体
            .replace("~~", "")  // 删除线
            .replace("`", "")   // 代码标记
            // 移除多余的空白字符
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 将HTML转换为纯文本 - 彻底清理HTML和CSS代码
    fn html_to_text(html: &str) -> String {
        let mut text = html.to_string();

        use regex::Regex;

        // 1. 移除CSS样式块和脚本
        let style_regex = Regex::new(r"<style[^>]*>[\s\S]*?</style>").unwrap();
        text = style_regex.replace_all(&text, "").to_string();
        let script_regex = Regex::new(r"<script[^>]*>[\s\S]*?</script>").unwrap();
        text = script_regex.replace_all(&text, "").to_string();

        // 2. 处理换行标签（在移除标签之前）
        text = text
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("</p>", "\n\n")
            .replace("</div>", "\n")
            .replace("</h1>", "\n\n")
            .replace("</h2>", "\n\n")
            .replace("</h3>", "\n\n")
            .replace("</h4>", "\n\n")
            .replace("</h5>", "\n\n")
            .replace("</h6>", "\n\n")
            .replace("</li>", "\n")
            .replace("</tr>", "\n")
            .replace("</td>", " ")
            .replace("</th>", " ");

        // 3. 移除所有HTML标签（包括不完整的标签）
        let html_tag_regex = Regex::new(r"<[^>]*>").unwrap();
        text = html_tag_regex.replace_all(&text, "").to_string();
        // 清理残留的不完整标签
        let incomplete_tag_regex = Regex::new(r"<[^<]*$").unwrap();
        text = incomplete_tag_regex.replace_all(&text, "").to_string();
        let incomplete_tag_start_regex = Regex::new(r"^[^>]*>").unwrap();
        text = incomplete_tag_start_regex.replace_all(&text, "").to_string();

        // 4. 强力清理CSS样式文本和字体声明
        // 清理字体族声明，如 'Segoe UI Semibold';'Segoe UI';
        let font_family_regex = Regex::new(r#"['"][^'"]*['"];?"#).unwrap();
        text = font_family_regex.replace_all(&text, " ").to_string();

        // 清理CSS选择器和规则
        let css_rule_regex = Regex::new(r"\s*[A-Za-z][A-Za-z0-9]*\s*\{[^}]*\}\s*").unwrap();
        text = css_rule_regex.replace_all(&text, " ").to_string();

        // 清理CSS属性
        let css_prop_regex = Regex::new(r"\s*[a-zA-Z-]+\s*:\s*[^;{}]+;?\s*").unwrap();
        text = css_prop_regex.replace_all(&text, " ").to_string();

        // 清理花括号内容
        let brace_regex = Regex::new(r"\s*\{[^}]*\}\s*").unwrap();
        text = brace_regex.replace_all(&text, " ").to_string();

        // 清理分号分隔的样式声明
        let semicolon_regex = Regex::new(r"[^;]*;[^;]*;").unwrap();
        text = semicolon_regex.replace_all(&text, " ").to_string();

        // 5. 清理特定的CSS模式
        // 清理CSS单位
        let css_unit_regex = Regex::new(r"\d+(\.\d+)?(px|em|rem|%|pt|pc|in|cm|mm|ex|ch|vw|vh|vmin|vmax);?").unwrap();
        text = css_unit_regex.replace_all(&text, " ").to_string();

        // 清理颜色值
        let color_hex_regex = Regex::new(r"#[0-9a-fA-F]{3,6};?").unwrap();
        text = color_hex_regex.replace_all(&text, " ").to_string();
        let color_rgb_regex = Regex::new(r"rgba?\([^)]+\);?").unwrap();
        text = color_rgb_regex.replace_all(&text, " ").to_string();

        // 6. 解码HTML实体
        text = text
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&apos;", "'")
            .replace("&copy;", "©")
            .replace("&reg;", "®")
            .replace("&trade;", "™");

        // 7. 移除特殊字符和符号
        let special_chars_regex = Regex::new(r"[{}();:]").unwrap();
        text = special_chars_regex.replace_all(&text, " ").to_string();

        // 8. 清理文本内容
        Self::clean_text_content(&text)
    }

    /// 创建TLS连接（带重试机制）
    async fn create_tls_connection(domain: &str, port: u16) -> anyhow::Result<TlsStream<TcpStream>> {
        let max_retries = if domain.contains("qq.com") { 5 } else { 3 };
        let mut last_error = None;

        for attempt in 1..=max_retries {
            tracing::debug!("TLS连接尝试 {}/{} - {}:{}", attempt, max_retries, domain, port);

            // 根据邮箱提供商调整超时时间
            let timeout_duration = if domain.contains("qq.com") {
                std::time::Duration::from_secs(45)
            } else {
                std::time::Duration::from_secs(30)
            };

            // 创建TCP连接（带超时）
            let tcp_stream = match tokio::time::timeout(
                timeout_duration,
                TcpStream::connect((domain, port))
            ).await {
                Ok(Ok(stream)) => {
                    tracing::debug!("TCP连接成功 - {}:{}", domain, port);
                    stream
                },
                Ok(Err(e)) => {
                    last_error = Some(anyhow::anyhow!("TCP连接失败: {}", e));
                    if attempt < max_retries {
                        let delay = attempt as u64 * 2;
                        tracing::warn!("TCP连接失败，{}秒后重试 ({}/{}): {}", delay, attempt, max_retries, e);
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        continue;
                    }
                    return Err(last_error.unwrap());
                },
                Err(_) => {
                    last_error = Some(anyhow::anyhow!("TCP连接超时"));
                    if attempt < max_retries {
                        let delay = attempt as u64 * 2;
                        tracing::warn!("TCP连接超时，{}秒后重试 ({}/{})", delay, attempt, max_retries);
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        continue;
                    }
                    return Err(last_error.unwrap());
                }
            };

            // 创建TLS连接（带超时）
            let tls_connector = TlsConnector::new();
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                tls_connector.connect(domain, tcp_stream)
            ).await {
                Ok(Ok(tls_stream)) => {
                    tracing::debug!("TLS握手成功 - {}:{} (尝试 {}/{})", domain, port, attempt, max_retries);
                    return Ok(tls_stream);
                },
                Ok(Err(e)) => {
                    last_error = Some(anyhow::anyhow!("TLS握手失败: {}", e));
                    if attempt < max_retries {
                        let delay = attempt as u64 * 2;
                        tracing::warn!("TLS握手失败，{}秒后重试 ({}/{}): {}", delay, attempt, max_retries, e);
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        continue;
                    }
                },
                Err(_) => {
                    last_error = Some(anyhow::anyhow!("TLS握手超时"));
                    if attempt < max_retries {
                        let delay = attempt as u64 * 2;
                        tracing::warn!("TLS握手超时，{}秒后重试 ({}/{})", delay, attempt, max_retries);
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        continue;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("TLS连接失败，已达到最大重试次数")))
    }



    /// 通过IMAP STORE命令标记邮件为已读/未读（带备用查找）
    pub async fn mark_as_read_via_imap_with_fallback(
        account: &crate::database::models::EmailAccount,
        message_id: &str,
        is_read: bool,
        subject: Option<&str>,
        from: Option<&str>,
        date: Option<&str>,
    ) -> anyhow::Result<bool> {
        use futures::stream::StreamExt;

        tracing::info!("开始通过IMAP标记邮件已读状态: {} -> {}", message_id, if is_read { "已读" } else { "未读" });

        // 检查是否是UUID格式（本地ID），如果是则直接返回成功
        // 因为真实的Message-ID应该在调用此函数之前就已经获取了
        if message_id.len() == 36 && message_id.chars().filter(|&c| c == '-').count() == 4 {
            tracing::warn!("传入的是UUID格式的本地ID，IMAP操作已跳过: {}", message_id);
            return Ok(true); // 返回成功，让本地缓存更新继续进行
        }

        // 建立TLS连接
        let tls_stream = Self::create_tls_connection(&account.imap_server, account.imap_port).await
            .context("创建TLS连接失败")?;

        let client = Client::new(tls_stream);

        // 根据认证类型选择登录方式
        let mut session = if account.auth_type.as_deref() == Some("oauth2") {
            // OAuth2认证
            if let Some(access_token) = &account.access_token {
                tracing::debug!("使用OAuth2认证进行IMAP登录");
                // Gmail使用邮箱地址而不是用户名
                let oauth_username = if account.imap_server.contains("gmail") {
                    &account.email
                } else {
                    &account.username
                };
                Self::oauth2_login(client, oauth_username, access_token).await
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

        // 选择收件箱文件夹
        session.select("INBOX").await
            .context("无法选择INBOX文件夹")?;

        // 查找邮件 - 优先使用快速方法
        let target_seq = if let (Some(subj), Some(sender), Some(_dt)) = (subject, from, date) {
            // 对于外部邮件，直接使用主题搜索（更快）
            tracing::info!("使用主题搜索查找邮件（快速模式）");
            match Self::find_email_by_metadata(&mut session, subj, sender, _dt).await {
                Some(seq) => {
                    tracing::info!("通过主题搜索找到邮件，序列号: {}", seq);
                    seq
                },
                None => {
                    // 如果主题搜索失败，再尝试Message-ID搜索
                    tracing::info!("主题搜索失败，尝试Message-ID搜索");
                    match Self::find_email_by_message_id(&mut session, message_id).await {
                        Some(seq) => seq,
                        None => {
                            tracing::warn!("所有查找方法都失败，跳过IMAP操作");
                            session.logout().await.context("IMAP登出失败")?;
                            return Ok(true);
                        }
                    }
                }
            }
        } else {
            // 对于没有元数据的邮件，使用Message-ID搜索
            match Self::find_email_by_message_id(&mut session, message_id).await {
                Some(seq) => seq,
                None => {
                    tracing::warn!("无法找到邮件，跳过IMAP操作");
                    session.logout().await.context("IMAP登出失败")?;
                    return Ok(true);
                }
            }
        };

        // 使用STORE命令标记邮件已读状态
        let flag_operation = if is_read {
            "+FLAGS (\\Seen)"  // 添加已读标记
        } else {
            "-FLAGS (\\Seen)"  // 移除已读标记
        };

        tracing::info!("执行STORE命令: {} {}", target_seq, flag_operation);

        // 执行STORE命令并处理响应
        let store_success = {
            match session.store(format!("{}", target_seq), flag_operation).await {
                Ok(mut store_stream) => {
                    // 消费所有响应
                    while let Some(_) = store_stream.next().await {
                        // 忽略响应内容
                    }
                    tracing::info!("STORE命令执行成功");
                    true
                },
                Err(e) => {
                    tracing::error!("STORE命令执行失败: {}", e);
                    false
                }
            }
        };

        // 登出
        session.logout().await.context("IMAP登出失败")?;

        if store_success {
            tracing::info!("成功通过IMAP标记邮件为{}: {}", if is_read { "已读" } else { "未读" }, message_id);
            Ok(true)
        } else {
            Err(anyhow::anyhow!("STORE命令执行失败"))
        }
    }

    /// 创建IMAP会话（通用方法）
    async fn create_imap_session(account: &EmailAccount) -> Result<Session<TlsStream<TcpStream>>> {
        // 建立TLS连接
        let tls_stream = Self::create_tls_connection(&account.imap_server, account.imap_port).await
            .context("创建TLS连接失败")?;

        let client = Client::new(tls_stream);

        // 根据认证类型选择登录方式
        let session = if account.auth_type.as_deref() == Some("oauth2") {
            // OAuth2认证
            if let Some(access_token) = &account.access_token {
                tracing::debug!("使用OAuth2认证进行IMAP登录");
                // Gmail使用邮箱地址而不是用户名
                let oauth_username = if account.imap_server.contains("gmail") {
                    &account.email
                } else {
                    &account.username
                };
                Self::oauth2_login(client, oauth_username, access_token).await
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

        Ok(session)
    }

    /// 解析邮件元数据（从message_id中提取序列号等信息）
    fn parse_message_metadata(_message_id: &str) -> Option<MessageMetadata> {
        // 如果message_id包含序列号信息，解析它
        // 这里可以根据实际的message_id格式来实现
        // 暂时返回None，表示没有元数据
        None
    }

    /// 使用IMAP FLAGS标记邮件星标状态
    pub async fn mark_message_as_starred_imap(
        account: &EmailAccount,
        message_id: &str,
        is_starred: bool,
    ) -> Result<bool> {
        use futures::stream::StreamExt;

        tracing::info!("开始IMAP标记邮件星标状态: {} -> {}", message_id, if is_starred { "星标" } else { "非星标" });

        // 检查是否是UUID格式（本地ID），如果是则直接返回失败
        if message_id.len() == 36 && message_id.chars().filter(|&c| c == '-').count() == 4 {
            tracing::warn!("传入的是UUID格式的本地ID，IMAP操作已跳过: {}", message_id);
            return Ok(false); // 返回false表示没有进行IMAP操作
        }

        // 建立IMAP连接
        let mut session = Self::create_imap_session(account).await
            .context("创建IMAP会话失败")?;

        // 选择收件箱
        session.select("INBOX").await
            .context("选择INBOX失败")?;

        // 查找邮件的序列号
        let target_seq = if let Some(metadata) = Self::parse_message_metadata(message_id) {
            metadata.sequence_number
        } else {
            // 对于没有元数据的邮件，使用快速Message-ID搜索
            match Self::find_email_by_message_id_fast(&mut session, message_id).await {
                Some(seq) => seq,
                None => {
                    tracing::warn!("无法找到邮件，跳过IMAP星标操作");
                    session.logout().await.context("IMAP登出失败")?;
                    return Ok(false); // 返回false表示没有成功标记
                }
            }
        };

        // 使用STORE命令标记邮件星标状态
        let flag_operation = if is_starred {
            "+FLAGS.SILENT (\\Flagged)"  // 添加星标标记
        } else {
            "-FLAGS.SILENT (\\Flagged)"  // 移除星标标记
        };

        tracing::info!("执行STORE命令: {} {}", target_seq, flag_operation);

        // 执行STORE命令并处理响应
        let store_success = {
            match session.store(format!("{}", target_seq), flag_operation).await {
                Ok(mut store_stream) => {
                    // 消费所有响应
                    while let Some(_) = store_stream.next().await {
                        // 忽略响应内容
                    }
                    tracing::info!("IMAP星标STORE命令执行成功");
                    true
                },
                Err(e) => {
                    tracing::error!("IMAP星标STORE命令执行失败: {}", e);
                    false
                }
            }
        };

        // 登出
        session.logout().await.context("IMAP登出失败")?;

        if store_success {
            tracing::info!("成功通过IMAP标记邮件为{}: {}", if is_starred { "星标" } else { "非星标" }, message_id);
            Ok(true)
        } else {
            Err(anyhow::anyhow!("IMAP星标STORE命令执行失败"))
        }
    }

    /// 通过IMAP保存草稿到草稿文件夹
    pub async fn save_draft_via_imap(
        account: &EmailAccount,
        request: &crate::SendEmailRequest
    ) -> Result<()> {


        // 创建IMAP连接（复制现有逻辑）
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

        // 使用用户名密码登录
        let mut session = client.login(&account.username, &account.password).await
            .map_err(|e| anyhow::anyhow!("IMAP登录失败: {:?}", e))?;

        // 构建邮件内容
        let email_content = Self::build_draft_email_content(request)?;

        // 尝试保存到草稿文件夹（QQ邮箱使用DRAFTS大写）
        let draft_folders = ["DRAFTS", "Drafts", "Draft", "草稿"];
        let mut saved = false;

        for folder_name in &draft_folders {
            match session.append(folder_name, Some("\\Draft"), None, email_content.as_bytes()).await {
                Ok(_) => {
                    tracing::info!("草稿已保存到文件夹: {}", folder_name);
                    saved = true;
                    break;
                },
                Err(e) => {
                    tracing::debug!("尝试保存到文件夹 {} 失败: {}", folder_name, e);
                    continue;
                }
            }
        }

        // 登出
        session.logout().await.context("IMAP登出失败")?;

        if saved {
            Ok(())
        } else {
            Err(anyhow::anyhow!("无法找到可用的草稿文件夹"))
        }
    }

    /// 构建草稿邮件内容
    fn build_draft_email_content(request: &crate::SendEmailRequest) -> Result<String> {
        let mut email_content = String::new();

        // 生成唯一的Message-ID
        let timestamp = chrono::Utc::now().timestamp_millis();
        let random_id = uuid::Uuid::new_v4().to_string().replace("-", "");
        let message_id = format!("draft-{}-{}@xmail.local", timestamp, &random_id[..8]);

        // 添加邮件头（包含唯一的Message-ID）
        email_content.push_str(&format!("Message-ID: <{}>\r\n", message_id));
        email_content.push_str(&format!("Subject: {}\r\n", request.subject));
        email_content.push_str(&format!("To: {}\r\n", request.to.join(", ")));

        if !request.cc.is_empty() {
            email_content.push_str(&format!("Cc: {}\r\n", request.cc.join(", ")));
        }

        if !request.bcc.is_empty() {
            email_content.push_str(&format!("Bcc: {}\r\n", request.bcc.join(", ")));
        }

        // 添加其他必要的头部
        email_content.push_str("MIME-Version: 1.0\r\n");
        email_content.push_str("Content-Type: text/plain; charset=utf-8\r\n");
        email_content.push_str("Content-Transfer-Encoding: 8bit\r\n");
        email_content.push_str("\r\n");

        // 添加邮件正文
        email_content.push_str(&request.body_text);

        Ok(email_content)
    }
}

/// 邮件元数据结构
struct MessageMetadata {
    sequence_number: u32,
}
