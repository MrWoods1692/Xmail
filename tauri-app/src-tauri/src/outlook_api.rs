use anyhow::{Result, Context};
use serde::Deserialize;
use crate::database::models::EmailMessage;
use chrono::{DateTime, Utc};

/// 从转发邮件内容中提取用户输入的评论部分
fn extract_user_comment_from_forward(body_text: &str) -> String {
    // 查找转发分隔符，只取分隔符之前的用户输入
    let separators = [
        "---------- 转发邮件 ----------",
        "-----原始邮件-----",
        "-------- Original Message --------",
        "\n\nFrom: ",
        "\n\nOn ",
    ];

    for separator in &separators {
        if let Some(index) = body_text.find(separator) {
            let user_comment = body_text[..index].trim();
            return user_comment.to_string();
        }
    }

    // 如果没有找到分隔符，返回全部内容（可能是纯用户输入）
    body_text.trim().to_string()
}

/// Microsoft Graph API 邮件服务
pub struct OutlookApiService;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GraphMailListResponse {
    value: Vec<GraphMailMessage>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GraphMailMessage {
    id: String,
    subject: Option<String>,
    #[serde(rename = "receivedDateTime")]
    received_date_time: String,
    #[serde(rename = "sentDateTime")]
    sent_date_time: Option<String>,
    from: Option<GraphEmailAddress>,
    #[serde(rename = "toRecipients")]
    to_recipients: Option<Vec<GraphEmailAddress>>,
    #[serde(rename = "ccRecipients")]
    cc_recipients: Option<Vec<GraphEmailAddress>>,
    #[serde(rename = "bccRecipients")]
    bcc_recipients: Option<Vec<GraphEmailAddress>>,
    body: Option<GraphMessageBody>,
    #[serde(rename = "hasAttachments")]
    has_attachments: Option<bool>,
    #[serde(rename = "isRead")]
    is_read: Option<bool>,
    #[serde(rename = "isDraft")]
    is_draft: Option<bool>,
    #[serde(rename = "parentFolderId")]
    parent_folder_id: Option<String>,
    flag: Option<GraphMessageFlag>,
}

#[derive(Debug, Deserialize)]
struct GraphMessageFlag {
    #[serde(rename = "flagStatus")]
    flag_status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphEmailAddress {
    #[serde(rename = "emailAddress")]
    email_address: GraphEmail,
}

#[derive(Debug, Deserialize)]
struct GraphEmail {
    address: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphMessageBody {
    #[serde(rename = "contentType")]
    content_type: String,
    content: String,
}

impl OutlookApiService {
    /// 获取Outlook邮件列表
    pub async fn fetch_messages(
        access_token: &str,
        folder: &str,
        limit: Option<i64>
    ) -> Result<Vec<EmailMessage>> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        // 将文件夹名称转换为Graph API文件夹
        let folder_name = match folder.to_uppercase().as_str() {
            "INBOX" => "inbox",
            "SENT" => "sentitems",
            "DRAFT" => "drafts",
            "DRAFTS" => "drafts",
            "SPAM" => "junkemail",
            "JUNK" => "junkemail",
            "TRASH" => "deleteditems",
            "DELETED" => "deleteditems",
            _ => {
                tracing::warn!("未知的Outlook文件夹: {}, 使用inbox", folder);
                "inbox"
            }
        };

        let max_results = limit.unwrap_or(200).min(1000); // Graph API最大1000，默认获取200封

        tracing::debug!("Graph API文件夹映射: {} -> {}", folder, folder_name);

        // 构建Graph API URL，包含flag字段以获取星标状态
        let list_url = format!(
            "https://graph.microsoft.com/v1.0/me/mailFolders/{}/messages?$top={}&$orderby=receivedDateTime desc&$select=id,subject,from,toRecipients,ccRecipients,bccRecipients,receivedDateTime,isRead,hasAttachments,body,flag",
            folder_name, max_results
        );

        if max_results <= 10 {
            tracing::debug!("获取Outlook邮件列表: {}", list_url);
        } else {
            tracing::info!("获取Outlook邮件列表: {}", list_url);
        }

        let list_response = client
            .get(&list_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .context("获取Outlook邮件列表失败")?;

        if !list_response.status().is_success() {
            let status = list_response.status();
            let error_text = list_response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Graph API请求失败: {} - {}",
                status,
                error_text
            ));
        }

        let list_data: GraphMailListResponse = list_response
            .json()
            .await
            .context("解析Outlook邮件列表响应失败")?;

        let graph_messages = list_data.value;

        if max_results <= 10 {
            tracing::debug!("获取到 {} 封邮件", graph_messages.len());
        } else {
            tracing::info!("获取到 {} 封邮件", graph_messages.len());
        }

        // 获取用户信息用于草稿邮件
        let user_email = Self::get_user_email(access_token).await.unwrap_or_default();

        // 转换为EmailMessage格式
        let mut messages = Vec::new();
        for graph_msg in graph_messages {
            match Self::convert_graph_message_to_email_message(graph_msg, folder, &user_email) {
                Ok(message) => messages.push(message),
                Err(e) => {
                    tracing::warn!("转换邮件格式失败: {}", e);
                    continue;
                }
            }
        }

        if max_results <= 10 {
            tracing::debug!("成功转换 {} 封邮件", messages.len());
        } else {
            tracing::info!("成功转换 {} 封邮件", messages.len());
        }

        Ok(messages)
    }

    /// 获取用户邮箱地址
    async fn get_user_email(access_token: &str) -> Result<String> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        let user_url = "https://graph.microsoft.com/v1.0/me";
        let response = client
            .get(user_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .context("获取用户信息失败")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("获取用户信息失败"));
        }

        let user_data: serde_json::Value = response
            .json()
            .await
            .context("解析用户信息失败")?;

        let email = user_data["mail"]
            .as_str()
            .or_else(|| user_data["userPrincipalName"].as_str())
            .unwrap_or_default()
            .to_string();

        Ok(email)
    }

    /// 删除Outlook邮件
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

        let delete_url = if is_in_trash {
            // 永久删除
            format!("https://graph.microsoft.com/v1.0/me/messages/{}", message_id)
        } else {
            // 移动到垃圾箱
            format!("https://graph.microsoft.com/v1.0/me/messages/{}/move", message_id)
        };

        let response = if is_in_trash {
            // 永久删除
            client
                .delete(&delete_url)
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await
                .context("永久删除邮件失败")?
        } else {
            // 移动到垃圾箱
            let move_request = serde_json::json!({
                "destinationId": "deleteditems"
            });

            client
                .post(&delete_url)
                .header("Authorization", format!("Bearer {}", access_token))
                .header("Content-Type", "application/json")
                .json(&move_request)
                .send()
                .await
                .context("移动邮件到垃圾箱失败")?
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "删除邮件失败: {} - {}",
                status,
                error_text
            ));
        }

        tracing::info!("成功删除邮件: {}", message_id);
        Ok(true)
    }

    /// 移动Outlook邮件到指定文件夹
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

        // 将文件夹名称转换为Graph API文件夹ID
        let destination_folder_id = match to_folder.to_uppercase().as_str() {
            "INBOX" => "inbox",
            "SENT" => "sentitems",
            "DRAFTS" => "drafts",
            "SPAM" => "junkemail",
            "TRASH" => "deleteditems",
            _ => "inbox", // 默认移动到收件箱
        };

        let move_url = format!("https://graph.microsoft.com/v1.0/me/messages/{}/move", message_id);
        let move_request = serde_json::json!({
            "destinationId": destination_folder_id
        });

        let response = client
            .post(&move_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&move_request)
            .send()
            .await
            .context("移动邮件失败")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "移动邮件失败: {} - {}",
                status,
                error_text
            ));
        }

        tracing::info!("成功移动邮件: {} 从 {} 到 {}", message_id, from_folder, to_folder);
        Ok(true)
    }

    /// 标记Outlook邮件为已读/未读
    pub async fn mark_as_read(
        access_token: &str,
        message_id: &str,
        is_read: bool,
    ) -> Result<bool> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        let update_url = format!("https://graph.microsoft.com/v1.0/me/messages/{}", message_id);
        let update_request = serde_json::json!({
            "isRead": is_read
        });

        let response = client
            .patch(&update_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&update_request)
            .send()
            .await
            .context("标记邮件已读状态失败")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "标记邮件已读状态失败: {} - {}",
                status,
                error_text
            ));
        }

        tracing::info!("成功标记邮件为{}: {}", if is_read { "已读" } else { "未读" }, message_id);
        Ok(true)
    }

    /// 标记Outlook邮件为星标/取消星标
    pub async fn mark_as_starred(
        access_token: &str,
        message_id: &str,
        is_starred: bool,
    ) -> Result<bool> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        let update_url = format!("https://graph.microsoft.com/v1.0/me/messages/{}", message_id);
        let update_request = serde_json::json!({
            "flag": {
                "flagStatus": if is_starred { "flagged" } else { "notFlagged" }
            }
        });

        let response = client
            .patch(&update_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&update_request)
            .send()
            .await
            .context("标记邮件星标状态失败")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "标记邮件星标状态失败: {} - {}",
                status,
                error_text
            ));
        }

        tracing::info!("成功标记邮件为{}: {}", if is_starred { "星标" } else { "非星标" }, message_id);
        Ok(true)
    }

    /// 将Graph API邮件转换为EmailMessage格式
    fn convert_graph_message_to_email_message(
        graph_msg: GraphMailMessage,
        folder: &str,
        user_email: &str,
    ) -> Result<EmailMessage> {
        // 解析接收时间
        let received_time = DateTime::parse_from_rfc3339(&graph_msg.received_date_time)
            .context("解析接收时间失败")?
            .with_timezone(&Utc);

        // 解析发送时间
        let _sent_time = if let Some(sent_str) = &graph_msg.sent_date_time {
            Some(DateTime::parse_from_rfc3339(sent_str)
                .context("解析发送时间失败")?
                .with_timezone(&Utc))
        } else {
            None
        };

        // 提取发件人
        let from_address = graph_msg.from
            .as_ref()
            .map(|f| f.email_address.address.clone())
            .unwrap_or_default();

        let from_name = graph_msg.from
            .as_ref()
            .and_then(|f| f.email_address.name.clone())
            .unwrap_or_default();

        // 对于草稿邮件，如果没有发件人信息，使用当前用户信息
        let (final_from_address, final_from_name) = if folder.to_uppercase() == "DRAFTS" && from_address.is_empty() {
            // 草稿邮件没有发件人信息，使用当前用户邮箱
            // 从邮箱地址提取用户名作为显示名称
            let display_name = if let Some(at_pos) = user_email.find('@') {
                user_email[..at_pos].to_string()
            } else {
                "我".to_string()
            };
            (user_email.to_string(), display_name)
        } else {
            (from_address.clone(), from_name.clone())
        };

        // 提取收件人
        let to_addresses: Vec<String> = graph_msg.to_recipients
            .unwrap_or_default()
            .iter()
            .map(|r| r.email_address.address.clone())
            .collect();

        // 调试日志：打印收件人信息
        if folder.to_uppercase() == "SENT" || folder.to_uppercase() == "SENTITEMS" {
            tracing::info!("已发送邮件收件人调试 - 邮件ID: {}, 主题: {:?}, 收件人: {:?}",
                graph_msg.id, graph_msg.subject, to_addresses);
        }

        // 提取抄送
        let cc_addresses: Vec<String> = graph_msg.cc_recipients
            .unwrap_or_default()
            .iter()
            .map(|r| r.email_address.address.clone())
            .collect();

        // 提取密送
        let bcc_addresses: Vec<String> = graph_msg.bcc_recipients
            .unwrap_or_default()
            .iter()
            .map(|r| r.email_address.address.clone())
            .collect();

        // 提取邮件内容
        let (body_text, body_html) = if let Some(body) = &graph_msg.body {
            match body.content_type.to_lowercase().as_str() {
                "html" => (String::new(), body.content.clone()),
                "text" => (body.content.clone(), String::new()),
                _ => (body.content.clone(), String::new()),
            }
        } else {
            (String::new(), String::new())
        };

        // 解析星标状态
        let is_starred = graph_msg.flag
            .as_ref()
            .and_then(|flag| flag.flag_status.as_ref())
            .map(|status| status.to_lowercase() == "flagged")
            .unwrap_or(false);

        Ok(EmailMessage {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: String::new(), // 将在调用处设置
            message_id: graph_msg.id,
            subject: graph_msg.subject.unwrap_or_default(),
            sender: if final_from_name.is_empty() { final_from_address.clone() } else { format!("{} <{}>", final_from_name, final_from_address) },
            recipients: to_addresses.join(", "),
            cc: if cc_addresses.is_empty() { None } else { Some(cc_addresses.join(", ")) },
            bcc: if bcc_addresses.is_empty() { None } else { Some(bcc_addresses.join(", ")) },
            body_text: if body_text.is_empty() { None } else { Some(body_text) },
            body_html: if body_html.is_empty() { None } else { Some(body_html) },
            folder: folder.to_string(),
            is_read: graph_msg.is_read.unwrap_or(false),
            is_starred: is_starred,
            is_deleted: false,
            received_at: received_time,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            imap_uid: None, // Outlook API不使用IMAP UID
        })
    }

    /// 使用Microsoft Graph API保存草稿
    pub async fn save_draft(
        access_token: &str,
        request: &crate::SendEmailRequest
    ) -> Result<()> {
        // 如果有draft_id，说明是更新现有草稿
        if let Some(draft_id) = &request.draft_id {
            return Self::update_draft(access_token, draft_id, request).await;
        }

        // 否则创建新草稿
        Self::create_new_draft(access_token, request).await
    }

    /// 创建新草稿
    async fn create_new_draft(
        access_token: &str,
        request: &crate::SendEmailRequest
    ) -> Result<()> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        // 构建草稿邮件数据 - 支持HTML格式
        let (content_type, content) = if let Some(html_content) = &request.body_html {
            if !html_content.trim().is_empty() {
                ("HTML", html_content.as_str())
            } else {
                ("Text", request.body_text.as_str())
            }
        } else {
            ("Text", request.body_text.as_str())
        };

        let mut message = serde_json::json!({
            "subject": request.subject,
            "body": {
                "contentType": content_type,
                "content": content
            },
            "toRecipients": request.to.iter().map(|email| {
                serde_json::json!({
                    "emailAddress": {
                        "address": email
                    }
                })
            }).collect::<Vec<_>>()
        });

        // 添加抄送
        if !request.cc.is_empty() {
            message["ccRecipients"] = serde_json::json!(
                request.cc.iter().map(|email| {
                    serde_json::json!({
                        "emailAddress": {
                            "address": email
                        }
                    })
                }).collect::<Vec<_>>()
            );
        }

        // 添加密送
        if !request.bcc.is_empty() {
            message["bccRecipients"] = serde_json::json!(
                request.bcc.iter().map(|email| {
                    serde_json::json!({
                        "emailAddress": {
                            "address": email
                        }
                    })
                }).collect::<Vec<_>>()
            );
        }

        // 如果是回复邮件，添加回复相关字段
        if let Some(in_reply_to) = &request.in_reply_to {
            // 注意：Microsoft Graph API 创建草稿时不直接支持 in_reply_to
            // 这里我们将信息保存在邮件内容中，或者使用自定义属性
            tracing::info!("草稿是回复邮件，原邮件ID: {}", in_reply_to);
        }

        let create_url = "https://graph.microsoft.com/v1.0/me/messages";

        tracing::info!("正在创建Outlook草稿邮件");
        let response = client
            .post(create_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&message)
            .send()
            .await
            .context("发送创建草稿请求失败")?;

        if response.status().is_success() {
            tracing::info!("Microsoft Graph API草稿创建成功");
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "无法读取错误响应".to_string());
            tracing::error!("Microsoft Graph API草稿创建失败: {}", error_text);
            Err(anyhow::anyhow!("草稿创建失败: {}", error_text))
        }
    }

    /// 更新现有草稿
    async fn update_draft(
        access_token: &str,
        draft_id: &str,
        request: &crate::SendEmailRequest
    ) -> Result<()> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        // 构建更新草稿邮件数据
        let mut message = serde_json::json!({
            "subject": request.subject,
            "body": {
                "contentType": "Text",
                "content": request.body_text
            },
            "toRecipients": request.to.iter().map(|email| {
                serde_json::json!({
                    "emailAddress": {
                        "address": email
                    }
                })
            }).collect::<Vec<_>>()
        });

        // 添加抄送
        if !request.cc.is_empty() {
            message["ccRecipients"] = serde_json::json!(
                request.cc.iter().map(|email| {
                    serde_json::json!({
                        "emailAddress": {
                            "address": email
                        }
                    })
                }).collect::<Vec<_>>()
            );
        }

        // 添加密送
        if !request.bcc.is_empty() {
            message["bccRecipients"] = serde_json::json!(
                request.bcc.iter().map(|email| {
                    serde_json::json!({
                        "emailAddress": {
                            "address": email
                        }
                    })
                }).collect::<Vec<_>>()
            );
        }

        let update_url = format!("https://graph.microsoft.com/v1.0/me/messages/{}", draft_id);

        tracing::info!("正在更新Outlook草稿邮件: {}", draft_id);
        let response = client
            .patch(&update_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&message)
            .send()
            .await
            .context("发送更新草稿请求失败")?;

        if response.status().is_success() {
            tracing::info!("Microsoft Graph API草稿更新成功");
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "无法读取错误响应".to_string());
            tracing::error!("Microsoft Graph API草稿更新失败: {}", error_text);
            Err(anyhow::anyhow!("草稿更新失败: {}", error_text))
        }
    }

    /// 使用Microsoft Graph API发送邮件
    pub async fn send_email(
        access_token: &str,
        request: &crate::SendEmailRequest
    ) -> Result<Vec<String>> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建HTTP客户端失败")?;

        // 构建邮件数据 - 支持HTML格式
        let (content_type, content) = if let Some(html_content) = &request.body_html {
            if !html_content.trim().is_empty() {
                ("HTML", html_content.as_str())
            } else {
                ("Text", request.body_text.as_str())
            }
        } else {
            ("Text", request.body_text.as_str())
        };

        let mut message = serde_json::json!({
            "subject": request.subject,
            "body": {
                "contentType": content_type,
                "content": content
            },
            "toRecipients": request.to.iter().map(|email| {
                serde_json::json!({
                    "emailAddress": {
                        "address": email
                    }
                })
            }).collect::<Vec<_>>()
        });

        // 添加抄送
        if !request.cc.is_empty() {
            message["ccRecipients"] = serde_json::json!(
                request.cc.iter().map(|email| {
                    serde_json::json!({
                        "emailAddress": {
                            "address": email
                        }
                    })
                }).collect::<Vec<_>>()
            );
        }

        // 添加密送
        if !request.bcc.is_empty() {
            message["bccRecipients"] = serde_json::json!(
                request.bcc.iter().map(|email| {
                    serde_json::json!({
                        "emailAddress": {
                            "address": email
                        }
                    })
                }).collect::<Vec<_>>()
            );
        }

        // 保存正确的收件人信息
        let correct_recipients = request.to.clone();

        // 如果是转发邮件，使用forward API让Outlook自动处理格式
        if request.is_forward.unwrap_or(false) {
            if let Some(forward_message_id) = &request.forward_message_id {
                tracing::info!("发送转发邮件，使用Outlook Forward API，原始邮件ID: {}", forward_message_id);

                // 使用Microsoft Graph API的转发功能
                // 注意：comment应该只包含用户输入，不包含转发格式，Outlook会自动添加原始邮件
                let user_comment = extract_user_comment_from_forward(&request.body_text);

                let forward_url = format!("https://graph.microsoft.com/v1.0/me/messages/{}/forward", forward_message_id);
                let forward_data = serde_json::json!({
                    "comment": user_comment,
                    "toRecipients": request.to.iter().map(|email| {
                        serde_json::json!({
                            "emailAddress": {
                                "address": email
                            }
                        })
                    }).collect::<Vec<_>>()
                });

                tracing::debug!("发送Outlook转发邮件: {}", forward_url);
                tracing::debug!("转发数据: {}", serde_json::to_string_pretty(&forward_data).unwrap_or_default());

                let response = client
                    .post(&forward_url)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .header("Content-Type", "application/json")
                    .json(&forward_data)
                    .send()
                    .await
                    .context("Microsoft Graph API发送转发邮件请求失败")?;

                if response.status().is_success() {
                    tracing::info!("Microsoft Graph API转发邮件发送成功");
                    return Ok(correct_recipients);
                } else {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();
                    tracing::error!("Microsoft Graph API发送转发邮件失败: {} - {}", status, error_text);
                    return Err(anyhow::anyhow!("Microsoft Graph API发送转发邮件失败: {} - {}", status, error_text));
                }
            } else {
                tracing::warn!("转发邮件缺少原始邮件ID，使用普通发送");
            }
        }
        // 如果是回复邮件，使用reply API让邮件客户端自动处理格式
        else if let Some(in_reply_to) = &request.in_reply_to {
            tracing::info!("发送回复邮件，原始邮件ID: {}, 收件人: {:?}", in_reply_to, request.to);

            // 使用reply API，同时指定comment和收件人
            let reply_url = format!("https://graph.microsoft.com/v1.0/me/messages/{}/reply", in_reply_to);
            let reply_data = serde_json::json!({
                "comment": request.body_text,
                "message": {
                    "toRecipients": request.to.iter().map(|email| {
                        serde_json::json!({
                            "emailAddress": {
                                "address": email
                            }
                        })
                    }).collect::<Vec<_>>()
                }
            });

            tracing::debug!("发送Outlook回复邮件: {}", reply_url);
            tracing::debug!("回复数据: {}", serde_json::to_string_pretty(&reply_data).unwrap_or_default());

            let response = client
                .post(&reply_url)
                .header("Authorization", format!("Bearer {}", access_token))
                .header("Content-Type", "application/json")
                .json(&reply_data)
                .send()
                .await
                .context("Microsoft Graph API发送回复邮件请求失败")?;

            if response.status().is_success() {
                tracing::info!("Microsoft Graph API回复邮件发送成功");
                return Ok(correct_recipients);
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("Microsoft Graph API发送回复邮件失败: {} - {}", status, error_text);
                return Err(anyhow::anyhow!("Microsoft Graph API发送邮件失败: {} - {}", status, error_text));
            }
        }


        // 普通邮件发送
        let send_data = serde_json::json!({
            "message": message
        });

        // 发送邮件
        let send_url = "https://graph.microsoft.com/v1.0/me/sendMail";

        tracing::debug!("发送Outlook邮件: {}", send_url);
        tracing::debug!("邮件数据: {}", serde_json::to_string_pretty(&send_data).unwrap_or_default());

        let response = client
            .post(send_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&send_data)
            .send()
            .await
            .context("Microsoft Graph API发送邮件请求失败")?;

        if response.status().is_success() {
            tracing::info!("Microsoft Graph API邮件发送成功");
            Ok(correct_recipients)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Microsoft Graph API发送邮件失败: {} - {}", status, error_text);

            // 保持原始错误信息以便上层进行令牌刷新判断
            Err(anyhow::anyhow!("Microsoft Graph API发送邮件失败: {} - {}", status, error_text))
        }
    }
}
