use anyhow::{Result, Context};
use serde::Deserialize;
use crate::database::models::EmailMessage;
use chrono::{DateTime, Utc};
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Deserialize)]
pub struct GmailMessage {
    pub id: String,
    #[serde(rename = "threadId")]
    #[allow(dead_code)]
    pub thread_id: String,
    #[serde(rename = "labelIds")]
    #[allow(dead_code)]
    pub label_ids: Option<Vec<String>>,
    #[allow(dead_code)]
    pub snippet: String,
    #[serde(rename = "historyId")]
    #[allow(dead_code)]
    pub history_id: Option<String>,
    #[serde(rename = "internalDate")]
    pub internal_date: Option<String>,
    pub payload: Option<GmailPayload>,
    #[serde(rename = "sizeEstimate")]
    #[allow(dead_code)]
    pub size_estimate: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GmailPayload {
    #[serde(rename = "partId")]
    #[allow(dead_code)]
    pub part_id: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    #[allow(dead_code)]
    pub filename: Option<String>,
    pub headers: Option<Vec<GmailHeader>>,
    pub body: Option<GmailBody>,
    pub parts: Option<Vec<GmailPayload>>,
}

#[derive(Debug, Deserialize)]
pub struct GmailHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct GmailBody {
    #[serde(rename = "attachmentId")]
    #[allow(dead_code)]
    pub attachment_id: Option<String>,
    #[allow(dead_code)]
    pub size: Option<i64>,
    pub data: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GmailListResponse {
    pub messages: Option<Vec<GmailMessageRef>>,
    #[serde(rename = "nextPageToken")]
    #[allow(dead_code)]
    pub next_page_token: Option<String>,
    #[serde(rename = "resultSizeEstimate")]
    #[allow(dead_code)]
    pub result_size_estimate: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GmailMessageRef {
    pub id: String,
    #[serde(rename = "threadId")]
    #[allow(dead_code)]
    pub thread_id: String,
}

pub struct GmailApiService;

impl GmailApiService {
    /// 修复Gmail API的base64url编码padding问题
    fn fix_base64url_padding(data: &str) -> String {
        let mut fixed_data = data.to_string();
        // 移除可能存在的换行符和空格
        fixed_data = fixed_data.replace('\n', "").replace('\r', "").replace(' ', "");

        // 添加必要的padding
        let padding_needed = (4 - (fixed_data.len() % 4)) % 4;
        for _ in 0..padding_needed {
            fixed_data.push('=');
        }

        fixed_data
    }

    /// 安全的base64url解码函数
    fn safe_base64url_decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
        // 首先尝试直接解码
        match general_purpose::URL_SAFE_NO_PAD.decode(data) {
            Ok(decoded) => Ok(decoded),
            Err(_) => {
                // 如果失败，尝试修复padding后再解码
                let fixed_data = Self::fix_base64url_padding(data);
                general_purpose::URL_SAFE.decode(fixed_data)
            }
        }
    }
    /// 获取Gmail邮件列表
    pub async fn fetch_messages(
        access_token: &str,
        folder: &str,
        limit: Option<i64>
    ) -> Result<Vec<EmailMessage>> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        // 将文件夹名称转换为Gmail标签
        let label_id = match folder.to_uppercase().as_str() {
            "INBOX" => "INBOX",
            "SENT" => "SENT",
            "DRAFT" => "DRAFT",
            "DRAFTS" => "DRAFT", // 支持复数形式，映射到Gmail的DRAFT标签
            "SPAM" => "SPAM",
            "JUNK" => "SPAM", // 垃圾邮件别名
            "TRASH" => "TRASH",
            "DELETED" => "TRASH", // 已删除别名
            _ => {
                tracing::warn!("未知的Gmail文件夹: {}, 使用INBOX", folder);
                "INBOX" // 默认使用收件箱
            }
        };

        let max_results = limit.unwrap_or(200).min(500); // Gmail API最大500，默认获取200封

        tracing::debug!("Gmail API文件夹映射: {} -> {}", folder, label_id);

        // 第一步：获取邮件ID列表
        let list_url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages?labelIds={}&maxResults={}",
            label_id, max_results
        );

        // 根据请求数量决定日志级别：少量邮件用DEBUG，避免过多INFO日志
        if max_results <= 10 {
            tracing::debug!("获取Gmail邮件列表: {}", list_url);
        } else {
            tracing::info!("获取Gmail邮件列表: {}", list_url);
        }

        let list_response = client
            .get(&list_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .context("获取Gmail邮件列表失败")?;

        if !list_response.status().is_success() {
            let status = list_response.status();
            let error_text = list_response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Gmail API请求失败: {} - {}",
                status,
                error_text
            ));
        }

        let list_data: GmailListResponse = list_response
            .json()
            .await
            .context("解析Gmail邮件列表响应失败")?;

        let message_refs = list_data.messages.unwrap_or_default();

        // 根据请求数量决定日志级别：少量邮件(<=10)用DEBUG，大量邮件用INFO
        if max_results <= 10 {
            tracing::debug!("获取到 {} 封邮件的ID", message_refs.len());
        } else {
            tracing::info!("获取到 {} 封邮件的ID", message_refs.len());
        }

        if message_refs.is_empty() {
            return Ok(vec![]);
        }

        // 第二步：并发获取邮件详情（提高性能）
        let message_refs_to_fetch: Vec<_> = message_refs.iter().take(max_results as usize).collect();

        if max_results <= 10 {
            tracing::debug!("开始并发获取 {} 封邮件详情", message_refs_to_fetch.len());
        } else {
            tracing::info!("开始并发获取 {} 封邮件详情", message_refs_to_fetch.len());
        }

        // 使用并发请求，但限制并发数量避免过载
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10)); // 最多10个并发请求
        let mut tasks = Vec::new();

        for message_ref in message_refs_to_fetch {
            let client = client.clone();
            let access_token = access_token.to_string();
            let message_id = message_ref.id.clone();
            let semaphore = semaphore.clone();

            let folder_clone = folder.to_string();
            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                Self::fetch_message_detail(&client, &access_token, &message_id, &folder_clone).await
            });

            tasks.push(task);
        }

        // 等待所有任务完成
        let mut messages = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(message)) => messages.push(message),
                Ok(Err(e)) => {
                    tracing::warn!("获取邮件详情失败: {}", e);
                    continue;
                },
                Err(e) => {
                    tracing::warn!("邮件详情任务失败: {}", e);
                    continue;
                }
            }
        }

        if max_results <= 10 {
            tracing::debug!("成功获取 {} 封邮件详情", messages.len());
        } else {
            tracing::info!("成功获取 {} 封邮件详情", messages.len());
        }
        Ok(messages)
    }

    /// 删除Gmail邮件
    pub async fn delete_message(
        access_token: &str,
        message_id: &str,
        current_folder: &str,
    ) -> Result<bool> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        // 检查是否在垃圾箱中
        let is_in_trash = current_folder.to_uppercase() == "TRASH" ||
                         current_folder.to_uppercase() == "DELETED";

        if is_in_trash {
            // 在垃圾箱中：永久删除邮件
            let delete_url = format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}",
                message_id
            );

            let response = client
                .delete(&delete_url)
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await
                .context("Gmail API删除邮件请求失败")?;

            if response.status().is_success() {
                tracing::info!("Gmail邮件已永久删除: {}", message_id);
                Ok(true)
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("Gmail API删除邮件失败: {} - {}", status, error_text);
                Err(anyhow::anyhow!("Gmail API删除邮件失败: {} - {}", status, error_text))
            }
        } else {
            // 在其他文件夹中：移动到垃圾箱
            let modify_url = format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}/modify",
                message_id
            );

            let modify_request = serde_json::json!({
                "addLabelIds": ["TRASH"],
                "removeLabelIds": [Self::folder_to_label_id(current_folder)]
            });

            let response = client
                .post(&modify_url)
                .header("Authorization", format!("Bearer {}", access_token))
                .header("Content-Type", "application/json")
                .json(&modify_request)
                .send()
                .await
                .context("Gmail API移动邮件到垃圾箱请求失败")?;

            if response.status().is_success() {
                tracing::info!("Gmail邮件已移动到垃圾箱: {}", message_id);
                Ok(true)
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("Gmail API移动邮件到垃圾箱失败: {} - {}", status, error_text);
                Err(anyhow::anyhow!("Gmail API移动邮件到垃圾箱失败: {} - {}", status, error_text))
            }
        }
    }

    /// 保存Gmail草稿
    pub async fn save_draft(
        _access_token: &str,
        _request: &crate::SendEmailRequest
    ) -> Result<()> {
        // TODO: 实现Gmail API保存草稿功能
        // Gmail API使用 POST https://gmail.googleapis.com/gmail/v1/users/me/drafts
        Err(anyhow::anyhow!("Gmail草稿保存功能尚未实现"))
    }

    /// 移动Gmail邮件到指定文件夹
    pub async fn move_message(
        access_token: &str,
        message_id: &str,
        from_folder: &str,
        to_folder: &str,
    ) -> Result<bool> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        let modify_url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}/modify",
            message_id
        );

        let from_label = Self::folder_to_label_id(from_folder);
        let to_label = Self::folder_to_label_id(to_folder);

        let modify_request = serde_json::json!({
            "addLabelIds": [to_label],
            "removeLabelIds": [from_label]
        });

        let response = client
            .post(&modify_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&modify_request)
            .send()
            .await
            .context("Gmail API移动邮件请求失败")?;

        if response.status().is_success() {
            tracing::info!("Gmail邮件已移动: {} 从 {} 到 {}", message_id, from_folder, to_folder);
            Ok(true)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Gmail API移动邮件失败: {} - {}", status, error_text);
            Err(anyhow::anyhow!("Gmail API移动邮件失败: {} - {}", status, error_text))
        }
    }

/// 标记Gmail邮件为已读/未读
pub async fn mark_as_read(
    access_token: &str,
    message_id: &str,
    is_read: bool,
) -> Result<bool> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .build()
        .context("创建HTTP客户端失败")?;

    let modify_url = format!(
        "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}/modify",
        message_id
    );

    let modify_request = if is_read {
        // 标记为已读：添加UNREAD标签
        serde_json::json!({
            "removeLabelIds": ["UNREAD"]
        })
    } else {
        // 标记为未读：移除UNREAD标签
        serde_json::json!({
            "addLabelIds": ["UNREAD"]
        })
    };

    let response = client
        .post(&modify_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&modify_request)
        .send()
        .await
        .context("Gmail API标记邮件已读状态请求失败")?;

    if response.status().is_success() {
        tracing::info!("Gmail邮件已标记为{}: {}", if is_read { "已读" } else { "未读" }, message_id);
        Ok(true)
    } else {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        tracing::error!("Gmail API标记邮件已读状态失败: {} - {}", status, error_text);
        Err(anyhow::anyhow!("Gmail API标记邮件已读状态失败: {} - {}", status, error_text))
    }
}

    /// 将文件夹名称转换为Gmail标签ID
    fn folder_to_label_id(folder: &str) -> &str {
        match folder.to_uppercase().as_str() {
            "INBOX" => "INBOX",
            "SENT" => "SENT",
            "DRAFT" | "DRAFTS" => "DRAFT",
            "SPAM" | "JUNK" => "SPAM",
            "TRASH" | "DELETED" => "TRASH",
            _ => "INBOX" // 默认使用收件箱
        }
    }

    /// 获取单个邮件的详细信息
    async fn fetch_message_detail(
        client: &reqwest::Client,
        access_token: &str,
        message_id: &str,
        folder: &str,
    ) -> Result<EmailMessage> {
        let detail_url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}?format=full",
            message_id
        );

        let detail_response = client
            .get(&detail_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .context("获取Gmail邮件详情失败")?;

        if !detail_response.status().is_success() {
            let status = detail_response.status();
            let error_text = detail_response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Gmail API邮件详情请求失败: {} - {}",
                status,
                error_text
            ));
        }

        let gmail_message: GmailMessage = detail_response
            .json()
            .await
            .context("解析Gmail邮件详情响应失败")?;

        Self::convert_gmail_message_to_email_message(gmail_message, folder)
    }

    /// 将Gmail API消息转换为我们的EmailMessage格式
    fn convert_gmail_message_to_email_message(gmail_message: GmailMessage, folder: &str) -> Result<EmailMessage> {
        let payload = gmail_message.payload.as_ref()
            .ok_or_else(|| anyhow::anyhow!("邮件缺少payload"))?;

        let empty_headers = vec![];
        let headers = payload.headers.as_ref().unwrap_or(&empty_headers);

        // 提取邮件头信息
        let subject = Self::get_header_value(headers, "Subject").unwrap_or_default();
        let from = Self::get_header_value(headers, "From").unwrap_or_default();
        let to = Self::get_header_value(headers, "To").unwrap_or_default();
        let _date_str = Self::get_header_value(headers, "Date").unwrap_or_default();
        let message_id = Self::get_header_value(headers, "Message-ID").unwrap_or_default();

        // 解析日期
        let received_time = if let Some(internal_date) = &gmail_message.internal_date {
            // internal_date是毫秒时间戳
            if let Ok(timestamp) = internal_date.parse::<i64>() {
                DateTime::from_timestamp(timestamp / 1000, ((timestamp % 1000) * 1_000_000) as u32)
                    .unwrap_or_else(|| Utc::now())
            } else {
                Utc::now()
            }
        } else {
            Utc::now()
        };

        // 提取邮件正文（增强版本，添加调试信息）
        tracing::debug!("开始提取邮件正文，邮件ID: {}", gmail_message.id);
        let body_text = Self::extract_body_text(payload);
        let body_html = Self::extract_body_html(payload);

        // 记录提取结果（只在DEBUG级别记录详细信息）
        match (&body_text, &body_html) {
            (Some(text), Some(html)) => {
                tracing::debug!("成功提取文本和HTML内容，文本长度: {}, HTML长度: {}", text.len(), html.len());
            },
            (Some(text), None) => {
                tracing::debug!("仅提取到文本内容，长度: {}", text.len());
            },
            (None, Some(html)) => {
                tracing::debug!("仅提取到HTML内容，长度: {}", html.len());
            },
            (None, None) => {
                // 降低日志级别，避免过多警告
                tracing::debug!("未能提取到邮件内容，邮件ID: {}", gmail_message.id);
                // 只在DEBUG级别记录详细的payload结构
                if tracing::enabled!(tracing::Level::DEBUG) {
                    if let Some(mime_type) = &payload.mime_type {
                        tracing::debug!("邮件MIME类型: {}", mime_type);
                    }
                    if let Some(parts) = &payload.parts {
                        tracing::debug!("邮件包含 {} 个部分", parts.len());
                        for (i, part) in parts.iter().enumerate() {
                            if let Some(part_mime) = &part.mime_type {
                                tracing::debug!("部分 {}: MIME类型 {}", i, part_mime);
                            }
                        }
                    }
                }
            }
        }

        // 如果没有提取到任何内容，使用默认值（降低日志级别）
        let final_body_text = body_text.unwrap_or_else(|| {
            tracing::debug!("使用默认空文本内容");
            String::new()
        });

        Ok(EmailMessage {
            id: gmail_message.id.clone(),
            account_id: "".to_string(), // 这个会在调用方设置
            message_id,
            subject,
            sender: from,
            recipients: to,
            cc: None,
            bcc: None,
            body_text: if final_body_text.is_empty() { None } else { Some(final_body_text) },
            body_html,
            folder: folder.to_string(),
            is_read: false,
            is_starred: false,
            is_deleted: false,
            received_at: received_time,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            imap_uid: None, // Gmail API不使用IMAP UID
        })
    }

    /// 从邮件头中获取指定字段的值
    fn get_header_value(headers: &[GmailHeader], name: &str) -> Option<String> {
        headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.clone())
    }

    /// 提取邮件正文文本（增强版本，支持更多MIME类型和嵌套结构）
    fn extract_body_text(payload: &GmailPayload) -> Option<String> {
        tracing::debug!("提取邮件正文文本，MIME类型: {:?}", payload.mime_type);

        // 如果有直接的body数据且是纯文本
        if let Some(mime_type) = &payload.mime_type {
            if mime_type == "text/plain" {
                if let Some(body) = &payload.body {
                    tracing::debug!("找到text/plain body，检查data字段");
                    if let Some(data) = &body.data {
                        tracing::debug!("data字段存在，长度: {}", data.len());
                        if !data.is_empty() {
                            match Self::safe_base64url_decode(data) {
                                Ok(decoded) => {
                                    tracing::debug!("base64解码成功，字节长度: {}", decoded.len());
                                    match String::from_utf8(decoded) {
                                        Ok(text) => {
                                            let cleaned_text = text.trim().to_string();
                                            if !cleaned_text.is_empty() {
                                                tracing::debug!("找到纯文本内容，长度: {}", cleaned_text.len());
                                                return Some(cleaned_text);
                                            } else {
                                                tracing::debug!("解码后的文本为空");
                                            }
                                        },
                                        Err(e) => {
                                            tracing::debug!("UTF-8解码失败: {}", e);
                                        }
                                    }
                                },
                                Err(e) => {
                                    tracing::debug!("base64解码失败: {}", e);
                                    // 只在DEBUG级别记录详细信息
                                    if tracing::enabled!(tracing::Level::DEBUG) {
                                        let preview = if data.len() > 100 { &data[..100] } else { data };
                                        tracing::debug!("原始data长度: {}, 内容预览: {}", data.len(), preview);
                                    }
                                }
                            }
                        } else {
                            tracing::debug!("data字段为空");
                        }
                    } else {
                        tracing::debug!("data字段不存在");
                    }
                } else {
                    tracing::debug!("body字段不存在");
                }
            }
        }

        // 如果有parts，递归查找内容
        if let Some(parts) = &payload.parts {
            tracing::debug!("处理多部分邮件，部分数量: {}", parts.len());

            // 首先尝试找到纯文本部分
            for (i, part) in parts.iter().enumerate() {
                tracing::debug!("处理部分 {}: MIME类型 {:?}", i, part.mime_type);

                if let Some(mime_type) = &part.mime_type {
                    if mime_type == "text/plain" {
                        if let Some(text) = Self::extract_body_text(part) {
                            tracing::debug!("在部分 {} 中找到纯文本内容", i);
                            return Some(text);
                        }
                    }
                    // 处理嵌套的multipart结构
                    else if mime_type.starts_with("multipart/") {
                        if let Some(text) = Self::extract_body_text(part) {
                            tracing::debug!("在嵌套multipart部分 {} 中找到文本内容", i);
                            return Some(text);
                        }
                    }
                }
            }

            // 如果没有找到纯文本，尝试从HTML中提取
            for (i, part) in parts.iter().enumerate() {
                if let Some(mime_type) = &part.mime_type {
                    if mime_type == "text/html" {
                        if let Some(html) = Self::extract_body_html(part) {
                            tracing::debug!("在部分 {} 中找到HTML内容，开始转换为文本", i);
                            let text = Self::html_to_text(&html);
                            if !text.trim().is_empty() {
                                tracing::debug!("HTML转文本成功，长度: {}", text.len());
                                return Some(text);
                            }
                        }
                    }
                }
            }
        }

        tracing::debug!("未能提取到邮件正文文本");
        None
    }

    /// 改进的HTML到文本转换函数 - 彻底清理HTML和CSS代码
    fn html_to_text(html: &str) -> String {
        let mut text = html.to_string();

        // 1. 移除CSS样式块和脚本
        let style_re = regex::Regex::new(r"<style[^>]*>[\s\S]*?</style>").unwrap();
        text = style_re.replace_all(&text, "").to_string();
        let script_re = regex::Regex::new(r"<script[^>]*>[\s\S]*?</script>").unwrap();
        text = script_re.replace_all(&text, "").to_string();

        // 2. 处理常见的HTML标签，保留换行结构
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
        let html_tag_re = regex::Regex::new(r"<[^>]*>").unwrap();
        text = html_tag_re.replace_all(&text, "").to_string();
        // 清理残留的不完整标签
        let incomplete_tag_re = regex::Regex::new(r"<[^<]*$").unwrap();
        text = incomplete_tag_re.replace_all(&text, "").to_string();
        let incomplete_tag_start_re = regex::Regex::new(r"^[^>]*>").unwrap();
        text = incomplete_tag_start_re.replace_all(&text, "").to_string();

        // 4. 强力清理CSS样式文本和字体声明
        // 清理字体族声明，如 'Segoe UI Semibold';'Segoe UI';
        let font_family_re = regex::Regex::new(r#"['"][^'"]*['"];?"#).unwrap();
        text = font_family_re.replace_all(&text, " ").to_string();

        // 清理CSS选择器和规则
        let css_rule_re = regex::Regex::new(r"\s*[A-Za-z][A-Za-z0-9]*\s*\{[^}]*\}\s*").unwrap();
        text = css_rule_re.replace_all(&text, " ").to_string();

        // 清理CSS属性
        let css_prop_re = regex::Regex::new(r"\s*[a-zA-Z-]+\s*:\s*[^;{}]+;?\s*").unwrap();
        text = css_prop_re.replace_all(&text, " ").to_string();

        // 清理花括号内容
        let brace_re = regex::Regex::new(r"\s*\{[^}]*\}\s*").unwrap();
        text = brace_re.replace_all(&text, " ").to_string();

        // 清理分号分隔的样式声明
        let semicolon_re = regex::Regex::new(r"[^;]*;[^;]*;").unwrap();
        text = semicolon_re.replace_all(&text, " ").to_string();

        // 5. 清理特定的CSS模式
        // 清理CSS单位
        let css_unit_re = regex::Regex::new(r"\d+(\.\d+)?(px|em|rem|%|pt|pc|in|cm|mm|ex|ch|vw|vh|vmin|vmax);?").unwrap();
        text = css_unit_re.replace_all(&text, " ").to_string();

        // 清理颜色值
        let color_hex_re = regex::Regex::new(r"#[0-9a-fA-F]{3,6};?").unwrap();
        text = color_hex_re.replace_all(&text, " ").to_string();
        let color_rgb_re = regex::Regex::new(r"rgba?\([^)]+\);?").unwrap();
        text = color_rgb_re.replace_all(&text, " ").to_string();

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
        let special_chars_re = regex::Regex::new(r"[{}();:]").unwrap();
        text = special_chars_re.replace_all(&text, " ").to_string();

        // 8. 清理多余的空白字符和换行
        let lines: Vec<&str> = text
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && line.len() > 2 && !line.chars().all(|c| c.is_whitespace() || c.is_ascii_punctuation()))
            .collect();

        let result = lines.join("\n");

        // 如果清理后内容太短或只包含无意义字符，返回空字符串
        if result.len() < 3 || result.chars().all(|c| c.is_whitespace() || c.is_ascii_punctuation()) {
            String::new()
        } else {
            result
        }
    }

    /// 提取邮件HTML内容（增强版本，支持更多嵌套结构）
    fn extract_body_html(payload: &GmailPayload) -> Option<String> {
        tracing::debug!("提取邮件HTML内容，MIME类型: {:?}", payload.mime_type);

        // 如果有直接的body数据且是HTML
        if let Some(mime_type) = &payload.mime_type {
            if mime_type == "text/html" {
                if let Some(body) = &payload.body {
                    if let Some(data) = &body.data {
                        if !data.is_empty() {
                            match Self::safe_base64url_decode(data) {
                                Ok(decoded) => {
                                    match String::from_utf8(decoded) {
                                        Ok(html) => {
                                            let cleaned_html = html.trim();
                                            if !cleaned_html.is_empty() {
                                                tracing::debug!("找到HTML内容，长度: {}", cleaned_html.len());
                                                return Some(cleaned_html.to_string());
                                            }
                                        },
                                        Err(e) => {
                                            tracing::debug!("HTML UTF-8解码失败: {}", e);
                                        }
                                    }
                                },
                                Err(e) => {
                                    tracing::debug!("HTML base64解码失败: {}", e);
                                    // 只在DEBUG级别记录详细信息
                                    if tracing::enabled!(tracing::Level::DEBUG) {
                                        let preview = if data.len() > 100 { &data[..100] } else { data };
                                        tracing::debug!("HTML原始data长度: {}, 内容预览: {}", data.len(), preview);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 如果有parts，递归查找HTML内容
        if let Some(parts) = &payload.parts {
            tracing::debug!("处理多部分邮件HTML，部分数量: {}", parts.len());

            // 如果是multipart/alternative，优先选择HTML
            if let Some(mime_type) = &payload.mime_type {
                if mime_type == "multipart/alternative" {
                    tracing::debug!("处理multipart/alternative，优先查找HTML");
                    // 先尝试HTML
                    for (i, part) in parts.iter().enumerate() {
                        if let Some(part_mime) = &part.mime_type {
                            if part_mime == "text/html" {
                                if let Some(html) = Self::extract_body_html(part) {
                                    tracing::debug!("在alternative部分 {} 中找到HTML内容", i);
                                    return Some(html);
                                }
                            }
                        }
                    }
                }
            }

            // 一般情况下递归查找HTML内容
            for (i, part) in parts.iter().enumerate() {
                if let Some(mime_type) = &part.mime_type {
                    if mime_type == "text/html" {
                        if let Some(html) = Self::extract_body_html(part) {
                            tracing::debug!("在部分 {} 中找到HTML内容", i);
                            return Some(html);
                        }
                    }
                    // 处理嵌套的multipart结构
                    else if mime_type.starts_with("multipart/") {
                        if let Some(html) = Self::extract_body_html(part) {
                            tracing::debug!("在嵌套multipart部分 {} 中找到HTML内容", i);
                            return Some(html);
                        }
                    }
                }
            }
        }

        tracing::debug!("未找到HTML内容");
        None
    }

    /// 发送Gmail邮件（暂未实现）
    pub async fn send_email(
        _access_token: &str,
        _request: &crate::SendEmailRequest
    ) -> Result<Vec<String>> {
        // TODO: 实现Gmail API发送邮件功能
        Err(anyhow::anyhow!("Gmail发送邮件功能尚未实现"))
    }

    /// 标记Gmail邮件为收藏/取消收藏（暂未实现）
    pub async fn mark_as_starred(
        _access_token: &str,
        _message_id: &str,
        _is_starred: bool,
    ) -> Result<bool> {
        // TODO: 实现Gmail API标记收藏功能
        Err(anyhow::anyhow!("Gmail标记收藏功能尚未实现"))
    }
}
