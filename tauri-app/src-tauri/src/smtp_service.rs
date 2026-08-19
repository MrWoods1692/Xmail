use anyhow::{Result, Context};
use crate::database::models::EmailAccount;
use crate::SendEmailRequest;
use crate::outlook_api::OutlookApiService;
use crate::gmail_api::GmailApiService;
use lettre::{
    Message, SmtpTransport, Transport,
    transport::smtp::authentication::Credentials,
    transport::smtp::client::{Tls, TlsParameters},
    Address,
    message::Mailbox
};
use std::str::FromStr;

/// SMTP邮件发送服务
pub struct SmtpService;

impl SmtpService {
    /// 发送邮件
    pub async fn send_email(account: &EmailAccount, request: &SendEmailRequest) -> Result<()> {
        tracing::info!("开始发送邮件: {} -> {:?}", account.email, request.to);

        // 根据邮箱域名判断账户类型
        let account_type = if account.email.contains("@outlook.com") ||
                             account.email.contains("@hotmail.com") ||
                             account.email.contains("@live.com") {
            "outlook"
        } else if account.email.contains("@gmail.com") {
            "gmail"
        } else if account.email.contains("@qq.com") {
            "qq"
        } else {
            "other"
        };

        // 根据账户类型选择发送方式
        match account_type {
            "outlook" => {
                Self::send_with_outlook_api(account, request).await
            },
            "gmail" => {
                Self::send_with_gmail_api(account, request).await
            },
            "qq" => {
                Self::send_with_qq_smtp(account, request).await
            },
            _ => {
                // 对于其他类型的邮箱，使用传统SMTP发送
                Self::send_with_traditional_smtp(account, request).await
            }
        }
    }

    /// 使用Outlook API发送邮件
    async fn send_with_outlook_api(account: &EmailAccount, request: &SendEmailRequest) -> Result<()> {
        let access_token = account.access_token.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Outlook账户缺少访问令牌"))?;

        // 添加调试日志来检查令牌格式
        tracing::info!("访问令牌长度: {}", access_token.len());
        tracing::info!("访问令牌前20个字符: {}", &access_token.chars().take(20).collect::<String>());
        tracing::info!("访问令牌是否包含点号: {}", access_token.contains('.'));

        let _correct_recipients = OutlookApiService::send_email(access_token, request).await
            .map_err(|e| {
                // 保持原始错误信息，确保401错误能被正确识别
                tracing::error!("Outlook API发送邮件失败: {}", e);
                e
            })?;

        tracing::info!("Outlook邮件发送成功");
        Ok(())
    }

    /// 使用Gmail API发送邮件
    async fn send_with_gmail_api(account: &EmailAccount, request: &SendEmailRequest) -> Result<()> {
        let access_token = account.access_token.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Gmail账户缺少访问令牌"))?;

        let _correct_recipients = GmailApiService::send_email(access_token, request).await
            .map_err(|e| {
                // 保持原始错误信息，确保401错误能被正确识别
                tracing::error!("Gmail API发送邮件失败: {}", e);
                e
            })?;

        tracing::info!("Gmail邮件发送成功");
        Ok(())
    }

    /// 使用QQ邮箱SMTP发送邮件
    async fn send_with_qq_smtp(account: &EmailAccount, request: &SendEmailRequest) -> Result<()> {
        tracing::info!("使用QQ邮箱SMTP发送邮件");

        // QQ邮箱特殊处理：使用SSL/TLS连接到465端口
        Self::send_with_traditional_smtp_internal(
            account,
            request,
            true, // QQ邮箱强制使用SSL
            Some("QQ邮箱")
        ).await
    }

    /// 使用传统SMTP发送邮件（适用于其他邮箱提供商）
    async fn send_with_traditional_smtp(account: &EmailAccount, request: &SendEmailRequest) -> Result<()> {
        tracing::info!("使用传统SMTP发送邮件");
        Self::send_with_traditional_smtp_internal(account, request, false, None).await
    }

    /// 传统SMTP发送的内部实现
    async fn send_with_traditional_smtp_internal(
        account: &EmailAccount,
        request: &SendEmailRequest,
        force_ssl: bool,
        provider_name: Option<&str>
    ) -> Result<()> {
        let provider = provider_name.unwrap_or("SMTP");
        tracing::info!("开始{}发送邮件: {} -> {:?}", provider, account.email, request.to);

        // 构建发件人地址 - 不使用显示名称，直接使用邮箱地址
        tracing::info!("原始发件人邮箱: {}, 显示名称: {}", account.email, account.name);
        let address = Address::from_str(&account.email)
            .context("无效的发件人邮箱地址")?;

        // 直接使用邮箱地址，不添加显示名称
        let from_mailbox = Mailbox::new(None, address);
        tracing::info!("解析后的发件人邮箱: {:?}", from_mailbox);

        // 构建邮件
        // 生成唯一的Message-ID（必须用尖括号包围）
        let message_id = format!("<{}@{}>",
            uuid::Uuid::new_v4().to_string().replace("-", ""),
            account.smtp_server
        );

        let mut message_builder = Message::builder()
            .from(from_mailbox)
            .subject(&request.subject)
            .date_now()
            .message_id(Some(message_id.clone()));

        tracing::info!("设置邮件Message-ID: {}", message_id);

        // 处理邮件类型相关的头部
        if request.is_forward.unwrap_or(false) {
            // 转发邮件：不设置In-Reply-To和References，避免邮件客户端自动添加引用
            tracing::info!("转发邮件：跳过设置回复头部，避免重复引用");
        } else {
            // 回复邮件或普通邮件：设置相应的头部
            if let Some(in_reply_to) = &request.in_reply_to {
                tracing::info!("设置回复邮件头: In-Reply-To = {}", in_reply_to);
                message_builder = message_builder.in_reply_to(in_reply_to.clone());
            }

            if let Some(references) = &request.references {
                tracing::info!("设置回复邮件头: References = {}", references);
                message_builder = message_builder.references(references.clone());
            }
        }

        // 添加收件人
        for to_email in &request.to {
            tracing::info!("原始收件人邮箱: {}", to_email);
            let to_address = Address::from_str(to_email)
                .context(format!("无效的收件人邮箱地址: {}", to_email))?;
            // 使用完整邮箱地址作为显示名称
            let to_mailbox = Mailbox::new(Some(to_email.clone()), to_address);
            tracing::info!("解析后的收件人邮箱: {:?}", to_mailbox);
            message_builder = message_builder.to(to_mailbox);
        }

        // 添加抄送
        for cc_email in &request.cc {
            let cc_address = Address::from_str(cc_email)
                .context(format!("无效的抄送邮箱地址: {}", cc_email))?;
            let cc_mailbox = Mailbox::new(Some(cc_email.clone()), cc_address);
            message_builder = message_builder.cc(cc_mailbox);
        }

        // 添加密送
        for bcc_email in &request.bcc {
            let bcc_address = Address::from_str(bcc_email)
                .context(format!("无效的密送邮箱地址: {}", bcc_email))?;
            let bcc_mailbox = Mailbox::new(Some(bcc_email.clone()), bcc_address);
            message_builder = message_builder.bcc(bcc_mailbox);
        }

        // 设置邮件内容（优先使用HTML格式）
        let message = if let Some(html_content) = &request.body_html {
            if !html_content.trim().is_empty() {
                message_builder.multipart(
                    lettre::message::MultiPart::alternative()
                        .singlepart(
                            lettre::message::SinglePart::builder()
                                .header(lettre::message::header::ContentType::TEXT_PLAIN)
                                .body(request.body_text.clone())
                        )
                        .singlepart(
                            lettre::message::SinglePart::builder()
                                .header(lettre::message::header::ContentType::TEXT_HTML)
                                .body(html_content.clone())
                        )
                )?
            } else {
                message_builder.body(request.body_text.clone())?
            }
        } else {
            message_builder.body(request.body_text.clone())?
        };

        // 创建SMTP传输
        let smtp_server = &account.smtp_server;
        let smtp_port = account.smtp_port;

        tracing::debug!("连接到{}服务器: {}:{}", provider, smtp_server, smtp_port);

        let mut transport_builder = SmtpTransport::relay(smtp_server)
            .context(format!("无法连接到{}服务器: {}", provider, smtp_server))?
            .port(smtp_port);

        // 根据配置和强制SSL设置选择加密方式
        if force_ssl || (account.use_tls && smtp_port == 465) {
            // 使用SSL/TLS (通常用于465端口)
            tracing::debug!("使用SSL/TLS连接");
            let tls_parameters = TlsParameters::new(smtp_server.clone())
                .context("创建TLS参数失败")?;
            transport_builder = transport_builder.tls(Tls::Wrapper(tls_parameters));
        } else if account.use_tls {
            // 使用STARTTLS (通常用于587端口)
            tracing::debug!("使用STARTTLS连接");
            let tls_parameters = TlsParameters::new(smtp_server.clone())
                .context("创建TLS参数失败")?;
            transport_builder = transport_builder.tls(Tls::Required(tls_parameters));
        } else {
            // 不使用加密
            tracing::debug!("使用明文连接");
            transport_builder = transport_builder.tls(Tls::None);
        }

        // 设置认证凭据
        let credentials = Credentials::new(account.username.clone(), account.password.clone());
        let transport = transport_builder
            .credentials(credentials)
            .build();

        // 发送邮件
        tracing::debug!("正在发送邮件...");
        match transport.send(&message) {
            Ok(response) => {
                tracing::info!("{}邮件发送成功: {:?}", provider, response);

                // 对于传统SMTP发送，尝试保存邮件到已发送文件夹
                if let Err(e) = Self::save_to_sent_folder(account, &message).await {
                    tracing::warn!("保存邮件到已发送文件夹失败: {}", e);
                    // 不影响发送成功的结果，只记录警告
                }

                Ok(())
            },
            Err(e) => {
                tracing::error!("{}邮件发送失败: {}", provider, e);
                Err(anyhow::anyhow!("{}邮件发送失败: {}", provider, e))
            }
        }
    }

    /// 保存邮件到已发送文件夹（通过IMAP）
    async fn save_to_sent_folder(account: &EmailAccount, message: &lettre::Message) -> Result<()> {
        use async_imap::Client;
        use async_std::net::TcpStream;
        use async_native_tls::TlsConnector;
        use futures::stream::StreamExt;

        tracing::info!("开始保存邮件到已发送文件夹");

        // 连接到IMAP服务器
        let domain = &account.imap_server;
        let port = account.imap_port;

        tracing::debug!("连接IMAP服务器: {}:{}", domain, port);

        // 创建TCP连接
        let tcp_stream = TcpStream::connect((domain.as_str(), port)).await
            .context("连接IMAP服务器失败")?;

        // 创建TLS连接
        let tls_connector = TlsConnector::new();
        let tls_stream = tls_connector.connect(domain, tcp_stream).await
            .context("TLS握手失败")?;

        let client = Client::new(tls_stream);

        // 登录IMAP服务器
        let mut session = client.login(&account.username, &account.password).await
            .map_err(|e| anyhow::anyhow!("IMAP登录失败: {:?}", e.0))?;

        tracing::debug!("IMAP登录成功");

        // 查找已发送文件夹
        let folders_stream = session.list(None, Some("*")).await
            .context("获取文件夹列表失败")?;

        let folders: Vec<_> = folders_stream.collect().await;
        let mut sent_folder = None;

        for folder_result in folders {
            if let Ok(folder) = folder_result {
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
            }
        }

        let sent_folder = sent_folder.unwrap_or_else(|| {
            tracing::warn!("未找到已发送文件夹，使用默认名称 'Sent'");
            "Sent".to_string()
        });

        // 将邮件转换为RFC822格式的字节数组
        let email_bytes = message.formatted();

        tracing::debug!("准备保存邮件到文件夹: {}", sent_folder);

        // 使用APPEND命令保存邮件到已发送文件夹
        // APPEND命令格式: APPEND mailbox [flags] [date-time] message
        let append_result = session.append(&sent_folder, None, None, &email_bytes).await;

        match append_result {
            Ok(_) => {
                tracing::info!("邮件已成功保存到已发送文件夹: {}", sent_folder);
            },
            Err(e) => {
                tracing::error!("保存邮件到已发送文件夹失败: {}", e);
                // 尝试创建已发送文件夹然后再保存
                tracing::info!("尝试创建已发送文件夹: {}", sent_folder);
                if let Err(create_err) = session.create(&sent_folder).await {
                    tracing::warn!("创建已发送文件夹失败: {}", create_err);
                } else {
                    tracing::info!("已发送文件夹创建成功，重新尝试保存邮件");
                    if let Err(retry_err) = session.append(&sent_folder, None, None, &email_bytes).await {
                        tracing::error!("重新保存邮件失败: {}", retry_err);
                        return Err(anyhow::anyhow!("保存邮件到已发送文件夹失败: {}", retry_err));
                    } else {
                        tracing::info!("邮件已成功保存到新创建的已发送文件夹");
                    }
                }
            }
        }

        // 登出
        session.logout().await.context("IMAP登出失败")?;

        Ok(())
    }
}


