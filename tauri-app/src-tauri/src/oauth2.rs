use anyhow::{Result, anyhow};
use rand::Rng;
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
use warp::Filter;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Token {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: String,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    token_type: String,
    scope: Option<String>,
}

pub struct OAuth2Client {
    config: OAuth2Config,
    client: reqwest::Client,
}

impl OAuth2Client {
    pub fn new(config: OAuth2Config) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// 生成授权URL（Gmail专用）
    pub fn get_authorization_url(&self, state: Option<String>) -> Result<String> {
        let state = state.unwrap_or_else(|| self.generate_state());

        let mut url = Url::parse(&self.config.auth_url)?;

        let query_params = vec![
            ("client_id", self.config.client_id.as_str()),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", self.config.scope.as_str()),
            ("state", &state),
            ("access_type", "offline"), // 获取refresh_token (Google专用)
            ("prompt", "consent"), // 强制显示同意页面
        ];

        url.query_pairs_mut().extend_pairs(query_params);

        Ok(url.to_string())
    }

    /// 生成授权URL（Outlook专用）
    pub fn get_outlook_authorization_url(&self, state: Option<String>) -> Result<String> {
        let state = state.unwrap_or_else(|| self.generate_state());

        let mut url = Url::parse(&self.config.auth_url)?;

        let query_params = vec![
            ("client_id", self.config.client_id.as_str()),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", self.config.scope.as_str()),
            ("state", &state),
            ("prompt", "consent"), // 强制显示同意页面 (Microsoft专用)
        ];

        url.query_pairs_mut().extend_pairs(query_params);

        Ok(url.to_string())
    }

    /// 使用授权码交换访问令牌
    pub async fn exchange_code_for_token(&self, code: &str) -> Result<OAuth2Token> {
        let mut params = HashMap::new();
        params.insert("client_id", self.config.client_id.as_str());
        params.insert("client_secret", self.config.client_secret.as_str());
        params.insert("code", code);
        params.insert("grant_type", "authorization_code");
        params.insert("redirect_uri", self.config.redirect_uri.as_str());
        params.insert("scope", &self.config.scope);

        let response = self.client
            .post(&self.config.token_url)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Token exchange failed: {}", error_text));
        }

        let token_response: TokenResponse = response.json().await?;
        
        Ok(OAuth2Token {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_in: token_response.expires_in,
            token_type: token_response.token_type,
            scope: token_response.scope,
        })
    }

    /// 刷新访问令牌（Gmail专用）
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<OAuth2Token> {
        let mut params = HashMap::new();
        params.insert("client_id", self.config.client_id.as_str());
        params.insert("client_secret", self.config.client_secret.as_str());
        params.insert("refresh_token", refresh_token);
        params.insert("grant_type", "refresh_token");
        params.insert("scope", &self.config.scope);

        let response = self.client
            .post(&self.config.token_url)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Token refresh failed: {}", error_text));
        }

        let token_response: TokenResponse = response.json().await?;

        Ok(OAuth2Token {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token.or(Some(refresh_token.to_string())), // 使用新的refresh_token，如果没有则保持原有的
            expires_in: token_response.expires_in,
            token_type: token_response.token_type,
            scope: token_response.scope,
        })
    }

    /// 刷新访问令牌（Outlook专用）
    pub async fn refresh_outlook_token(&self, refresh_token: &str) -> Result<OAuth2Token> {
        let mut params = HashMap::new();
        params.insert("client_id", self.config.client_id.as_str());
        // 对于Microsoft公共客户端，不包含client_secret
        params.insert("refresh_token", refresh_token);
        params.insert("grant_type", "refresh_token");
        params.insert("scope", &self.config.scope);

        let response = self.client
            .post(&self.config.token_url)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Token refresh failed: {}", error_text));
        }

        let token_response: TokenResponse = response.json().await?;

        Ok(OAuth2Token {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token.or(Some(refresh_token.to_string())), // 使用新的refresh_token，如果没有则保持原有的
            expires_in: token_response.expires_in,
            token_type: token_response.token_type,
            scope: token_response.scope,
        })
    }

    /// 生成随机状态字符串
    fn generate_state(&self) -> String {
        let mut rng = rand::rng();
        (0..32)
            .map(|_| {
                let idx = rng.random_range(0..62);
                match idx {
                    0..=25 => (b'A' + idx) as char,
                    26..=51 => (b'a' + (idx - 26)) as char,
                    _ => (b'0' + (idx - 52)) as char,
                }
            })
            .collect()
    }
}

/// Gmail OAuth2 配置 - 用于用户登录
pub fn get_gmail_oauth2_config() -> OAuth2Config {
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .unwrap_or_else(|_| "your-client-id.apps.googleusercontent.com".to_string());
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
        .unwrap_or_else(|_| "your-client-secret".to_string());

    // 调试信息
    tracing::info!("快捷登录OAuth2配置 - Client ID: {}", client_id);
    tracing::info!("快捷登录OAuth2配置 - Client Secret: {}", if client_secret.starts_with("GOCSPX-") { "已设置" } else { "未设置或无效" });

    OAuth2Config {
        client_id,
        client_secret,
        redirect_uri: "http://127.0.0.1:8081".to_string(), // 使用与邮件功能相同的端口
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_url: "https://oauth2.googleapis.com/token".to_string(),
        scope: "https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile".to_string(), // 用户登录的基本权限
    }
}

/// Outlook OAuth2 配置
pub fn get_outlook_oauth2_config() -> OAuth2Config {
    OAuth2Config {
        client_id: "3208b8e1-bf68-4002-9820-674f1a9b0cd4".to_string(),
        client_secret: "".to_string(), // 公共客户端不需要密钥
        redirect_uri: "http://localhost:8081/auth/callback".to_string(),
        auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
        token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
        scope: "https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/User.Read https://graph.microsoft.com/IMAP.AccessAsUser.All offline_access".to_string(),
    }
}

/// Gmail OAuth2 配置 - 专用于邮件访问
pub fn get_gmail_mail_oauth2_config() -> OAuth2Config {
    OAuth2Config {
        client_id: std::env::var("GOOGLE_CLIENT_ID")
            .unwrap_or_else(|_| "your-client-id.apps.googleusercontent.com".to_string()),
        client_secret: std::env::var("GOOGLE_CLIENT_SECRET")
            .unwrap_or_else(|_| "your-client-secret".to_string()),
        redirect_uri: "http://127.0.0.1:8081".to_string(), // 邮件功能使用不同端口
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_url: "https://oauth2.googleapis.com/token".to_string(),
        scope: "https://mail.google.com/".to_string(), // 邮件访问权限
    }
}

/// 验证访问令牌是否有效
pub async fn verify_gmail_token(access_token: &str) -> Result<bool> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://www.googleapis.com/oauth2/v1/tokeninfo")
        .query(&[("access_token", access_token)])
        .send()
        .await?;

    Ok(response.status().is_success())
}

/// 启动本地HTTP服务器并完成OAuth2授权流程
pub async fn complete_oauth2_flow() -> Result<OAuth2Token, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let config = get_gmail_mail_oauth2_config();
    let client = OAuth2Client::new(config.clone());

    // 生成授权URL
    let auth_url = client.get_authorization_url(None)
        .map_err(|e| format!("生成授权URL失败: {}", e))?;

    // 使用Arc<Mutex<Option<String>>>来存储授权码
    let auth_code = Arc::new(std::sync::Mutex::new(None::<String>));
    let server_should_stop = Arc::new(AtomicBool::new(false));

    // 启动本地HTTP服务器
    let auth_code_clone = auth_code.clone();
    let server_should_stop_clone = server_should_stop.clone();

    let callback_route = warp::path::end()
        .and(warp::query::<HashMap<String, String>>())
        .map(move |params: HashMap<String, String>| {
            let auth_code = auth_code_clone.clone();
            let server_should_stop = server_should_stop_clone.clone();

            if let Some(code) = params.get("code") {
                // 存储授权码
                if let Ok(mut guard) = auth_code.lock() {
                    *guard = Some(code.clone());
                }
                server_should_stop.store(true, Ordering::Relaxed);

                warp::reply::html(
                    r#"
                    <html>
                        <head>
                            <title>Gmail授权成功</title>
                        </head>
                        <body>
                            <script>
                                // 立即尝试关闭窗口
                                function tryClose() {
                                    try {
                                        // 方法1: 直接关闭
                                        window.close();
                                        return true;
                                    } catch (e) {
                                        try {
                                            // 方法2: 先打开空白页再关闭
                                            window.open('', '_self');
                                            window.close();
                                            return true;
                                        } catch (e2) {
                                            try {
                                                // 方法3: 使用history.back()
                                                if (window.history.length > 1) {
                                                    window.history.back();
                                                    setTimeout(() => window.close(), 100);
                                                    return true;
                                                }
                                            } catch (e3) {
                                                // 最后尝试：重定向到空白页
                                                window.location.href = 'about:blank';
                                                return false;
                                            }
                                        }
                                    }
                                    return false;
                                }

                                // 页面加载完成后立即尝试关闭
                                window.addEventListener('load', () => {
                                    setTimeout(tryClose, 100);
                                });

                                // 立即尝试关闭
                                tryClose();
                            </script>
                        </body>
                    </html>
                    "#.to_string()
                )
            } else if let Some(error) = params.get("error") {
                server_should_stop.store(true, Ordering::Relaxed);
                warp::reply::html(format!(
                    r#"
                    <html>
                        <head><title>授权失败</title></head>
                        <body style="font-family: Arial, sans-serif; text-align: center; padding: 50px;">
                            <h1 style="color: #f44336;">❌ 授权失败</h1>
                            <p>错误: {}</p>
                            <p>请关闭此页面并重试。</p>
                        </body>
                    </html>
                    "#, error
                ))
            } else {
                warp::reply::html(
                    r#"
                    <html>
                        <head><title>无效请求</title></head>
                        <body style="font-family: Arial, sans-serif; text-align: center; padding: 50px;">
                            <h1 style="color: #ff9800;">⚠️ 无效请求</h1>
                            <p>未收到有效的授权码。</p>
                        </body>
                    </html>
                    "#.to_string()
                )
            }
        });

    // 使用系统命令打开新的浏览器窗口（而不是标签页）
    let browser_opened = if cfg!(target_os = "windows") {
        // Windows: 尝试使用默认浏览器打开新窗口
        // 方法1: 尝试使用rundll32调用默认浏览器的新窗口模式
        let rundll_result = std::process::Command::new("rundll32")
            .args(&["url.dll,FileProtocolHandler", &auth_url])
            .spawn();

        if rundll_result.is_ok() {
            tracing::info!("使用rundll32打开默认浏览器");
            true
        } else {
            // 方法2: 尝试常见浏览器的新窗口模式
            // 检测并使用Chrome
            let chrome_paths = [
                "chrome",
                "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
                "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe"
            ];

            let mut chrome_opened = false;
            for chrome_path in &chrome_paths {
                if let Ok(_) = std::process::Command::new(chrome_path)
                    .args(&["--new-window", &auth_url])
                    .spawn() {
                    tracing::info!("使用Chrome打开新窗口: {}", chrome_path);
                    chrome_opened = true;
                    break;
                }
            }

            if chrome_opened {
                true
            } else {
                // 尝试Edge
                if let Ok(_) = std::process::Command::new("msedge")
                    .args(&["--new-window", &auth_url])
                    .spawn() {
                    tracing::info!("使用Edge打开新窗口");
                    true
                } else {
                    // 尝试Firefox
                    let firefox_paths = [
                        "firefox",
                        "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
                        "C:\\Program Files (x86)\\Mozilla Firefox\\firefox.exe"
                    ];

                    let mut firefox_opened = false;
                    for firefox_path in &firefox_paths {
                        if let Ok(_) = std::process::Command::new(firefox_path)
                            .args(&["--new-window", &auth_url])
                            .spawn() {
                            tracing::info!("使用Firefox打开新窗口: {}", firefox_path);
                            firefox_opened = true;
                            break;
                        }
                    }

                    firefox_opened
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        // macOS: 使用open命令打开新窗口
        std::process::Command::new("open")
            .args(&["-n", &auth_url])
            .spawn()
            .is_ok()
    } else {
        // Linux: 尝试使用xdg-open或常见浏览器
        std::process::Command::new("xdg-open")
            .arg(&auth_url)
            .spawn()
            .or_else(|_| {
                std::process::Command::new("google-chrome")
                    .args(&["--new-window", &auth_url])
                    .spawn()
            })
            .or_else(|_| {
                std::process::Command::new("firefox")
                    .args(&["--new-window", &auth_url])
                    .spawn()
            })
            .is_ok()
    };

    // 如果无法使用新窗口模式，回退到默认方式
    if !browser_opened {
        tracing::warn!("无法打开新浏览器窗口，回退到默认方式");
        if let Err(e) = opener::open(&auth_url) {
            return Err(format!("无法打开浏览器: {}", e));
        }
    } else {
        tracing::info!("已在新浏览器窗口中打开OAuth2授权页面");
    }

    // 从redirect_uri中提取端口号
    let port = if let Ok(url) = url::Url::parse(&config.redirect_uri) {
        url.port().unwrap_or(8080)
    } else {
        8080
    };

    // 在指定端口启动服务器（异步启动）
    let server_task = tokio::spawn(async move {
        warp::serve(callback_route)
            .run(([127, 0, 0, 1], port))
            .await;
    });

    // 等待授权码或超时
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(120); // 2分钟超时，更合理

    let code = loop {
        // 检查是否收到授权码
        if let Ok(guard) = auth_code.lock() {
            if let Some(code) = guard.as_ref() {
                // 收到授权码，停止服务器
                server_task.abort();
                break code.clone();
            }
        }

        // 检查是否应该停止服务器（收到错误或用户取消）
        if server_should_stop.load(std::sync::atomic::Ordering::Relaxed) {
            server_task.abort();
            // 如果没有授权码但服务器应该停止，说明出现了错误
            if let Ok(guard) = auth_code.lock() {
                if guard.is_none() {
                    return Err("授权被取消或失败".to_string());
                }
            }
        }

        // 检查超时
        if start_time.elapsed() > timeout {
            server_task.abort();
            return Err("授权超时（2分钟），请重试".to_string());
        }

        // 短暂休眠避免忙等待
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    };

    // 使用授权码交换访问令牌
    client.exchange_code_for_token(&code).await
        .map_err(|e| format!("令牌交换失败: {}", e))
}
