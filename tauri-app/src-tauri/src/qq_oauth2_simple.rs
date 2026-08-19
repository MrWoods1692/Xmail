// QQ OAuth2 simplified module
use serde::{Deserialize, Serialize};
use reqwest;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Emitter};
use tokio::net::TcpListener;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QQUserInfo {
    pub openid: String,
    pub nickname: String,
    pub avatar: String,
    pub gender: Option<String>,
    pub vip: Option<String>,
    pub level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QQLoginResult {
    pub success: bool,
    pub message: String,
    pub user_info: Option<QQUserInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QQApiUserData {
    pub openid: String,
    pub nickname: String,
    pub figureurl: Option<String>,
    pub figureurl_1: Option<String>,
    pub figureurl_2: Option<String>,
    pub figureurl_qq_1: Option<String>,
    pub figureurl_qq_2: Option<String>,
    pub gender: String,
    pub is_yellow_vip: Option<String>,
    pub vip: String,
    pub yellow_vip_level: Option<String>,
    pub level: String,
    pub is_yellow_year_vip: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QQSimpleUserResponse {
    pub ret: i32,
    pub msg: String,
    pub data: Option<QQApiUserData>,
}

#[tauri::command]
pub async fn start_qq_oauth2_login(app_handle: AppHandle) -> Result<QQLoginResult, String> {
    println!("Starting QQ OAuth2 login flow (simplified mode)...");

    // 启动本地HTTP服务器
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        if let Err(e) = start_callback_server(app_handle_clone).await {
            println!("Failed to start callback server: {}", e);
        }
    });

    // 等待一下让服务器启动
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let callback_url = "http://localhost:8080/qq_callback";
    let simple_url = format!(
        "https://xmail.34567.xin/api/auth/qq/simple?callback={}&msg=desktop_login",
        urlencoding::encode(callback_url)
    );

    println!("QQ simplified login URL: {}", simple_url);

    match create_qq_auth_window(&app_handle, &simple_url).await {
        Ok(_) => {
            println!("QQ authorization window opened");
            Ok(QQLoginResult {
                success: false,
                message: "请在授权窗口中完成QQ登录".to_string(),
                user_info: None,
            })
        }
        Err(e) => {
            println!("Failed to create authorization window: {}", e);
            Err(e)
        }
    }
}

async fn create_qq_auth_window(app_handle: &AppHandle, auth_url: &str) -> Result<(), String> {
    if let Some(existing_window) = app_handle.get_webview_window("qq_auth") {
        let _ = existing_window.close();
    }

    let window = WebviewWindowBuilder::new(
        app_handle,
        "qq_auth",
        WebviewUrl::External(auth_url.parse().map_err(|e| format!("Invalid URL: {}", e))?),
    )
    .title("QQ登录授权")
    .inner_size(500.0, 700.0)
    .center()
    .resizable(true)
    .minimizable(false)
    .maximizable(false)
    .closable(true)
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
    .build()
    .map_err(|e| format!("Failed to create window: {}", e))?;

    let app_handle_clone = app_handle.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            println!("QQ authorization window closed by user");
            let _ = app_handle_clone.emit("qq-login-error", "用户取消了QQ授权");
        }
    });

    println!("QQ authorization window created successfully");
    Ok(())
}

async fn start_callback_server(app_handle: AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("QQ callback server started on http://localhost:8080");

    loop {
        let (stream, _) = listener.accept().await?;
        let app_handle_clone = app_handle.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_http_request(stream, app_handle_clone).await {
                println!("Error handling HTTP request: {}", e);
            }
        });
    }
}

async fn handle_http_request(
    mut stream: tokio::net::TcpStream,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..n]);

    println!("Received HTTP request: {}", request);

    // 解析请求行
    let first_line = request.lines().next().unwrap_or("");
    if first_line.starts_with("GET /qq_callback") || first_line.starts_with("GET /qq") {
        // 解析查询参数
        if let Some(query_start) = first_line.find('?') {
            let query_end = first_line[query_start..].find(' ').map(|i| i + query_start).unwrap_or(first_line.len());
            let query_string = &first_line[query_start + 1..query_end];

            println!("Query string: {}", query_string);

            // 解析参数
            let params: std::collections::HashMap<String, String> = query_string
                .split('&')
                .filter_map(|pair| {
                    let mut parts = pair.split('=');
                    if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                        Some((
                            urlencoding::decode(key).unwrap_or_default().to_string(),
                            urlencoding::decode(value).unwrap_or_default().to_string(),
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            // 处理QQ回调
            let result = if let Some(code) = params.get("code") {
                println!("Received QQ authorization code: {}", code);

                match get_qq_user_info(code).await {
                    Ok(user_info) => {
                        println!("QQ login successful, user: {}", user_info.nickname);

                        // 发送成功事件
                        let _ = app_handle.emit("qq-login-success", &user_info);

                        // 延迟关闭授权窗口，确保事件已发送
                        let app_handle_clone = app_handle.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            if let Some(auth_window) = app_handle_clone.get_webview_window("qq_auth") {
                                let _ = auth_window.close();
                            }
                        });

                        // 返回简单的成功页面，但窗口会很快自动关闭
                        r#"<html><body><script>window.close();</script></body></html>"#.to_string()
                    }
                    Err(e) => {
                        println!("Failed to get user info: {}", e);
                        let _ = app_handle.emit("qq-login-error", e.clone());
                        format!("登录失败：{}", e)
                    }
                }
            } else if let Some(error) = params.get("error") {
                println!("QQ authorization failed: {}", error);
                let _ = app_handle.emit("qq-login-error", error.clone());
                format!("授权失败：{}", error)
            } else {
                let error_msg = "未收到有效的授权码或错误信息";
                println!("{}", error_msg);
                let _ = app_handle.emit("qq-login-error", error_msg);
                error_msg.to_string()
            };

            // 发送HTTP响应
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><body><h1>{}</h1><script>setTimeout(() => window.close(), 2000);</script></body></html>",
                result
            );

            stream.write_all(response.as_bytes()).await?;
        }
    }

    Ok(())
}

async fn get_qq_user_info(code: &str) -> Result<QQUserInfo, String> {
    let client = reqwest::Client::new();
    let api_url = format!("https://xmail.34567.xin/api/auth/qq/get_user_info?code={}", code);

    println!("Getting user info: {}", api_url);

    match client.get(&api_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                // 先获取原始文本来调试
                match response.text().await {
                    Ok(response_text) => {
                        println!("API response text: {}", response_text);

                        // 尝试解析JSON
                        match serde_json::from_str::<QQSimpleUserResponse>(&response_text) {
                            Ok(user_response) => {
                                if user_response.ret == 0 && user_response.msg == "success" {
                                    if let Some(data) = user_response.data {
                                        // 成功获取用户信息，转换为QQUserInfo格式
                                        let user_info = QQUserInfo {
                                            openid: data.openid,
                                            nickname: data.nickname,
                                            avatar: data.figureurl_2.or(data.figureurl_1).or(data.figureurl).unwrap_or_default(),
                                            gender: Some(data.gender),
                                            vip: Some(data.vip),
                                            level: Some(data.level),
                                        };
                                        Ok(user_info)
                                    } else {
                                        Err("API返回成功但没有用户数据".to_string())
                                    }
                                } else {
                                    Err(format!("API返回错误：ret={}, msg={}", user_response.ret, user_response.msg))
                                }
                            }
                            Err(e) => {
                                println!("JSON parse error: {}", e);
                                Err(format!("解析用户信息失败：{}", e))
                            }
                        }
                    }
                    Err(e) => Err(format!("读取响应文本失败：{}", e))
                }
            } else {
                Err(format!("API请求失败，状态码：{}", response.status()))
            }
        }
        Err(e) => Err(format!("网络请求失败：{}", e))
    }
}

#[tauri::command]
pub async fn handle_qq_protocol_callback(
    _app_handle: AppHandle,
    _data: String,
) -> Result<QQLoginResult, String> {
    // 这个函数现在不再使用，因为我们使用HTTP回调
    // HTTP回调由本地服务器处理
    Err("This method is deprecated, using HTTP callback instead".to_string())
}

#[tauri::command]
pub async fn handle_qq_callback_legacy(
    _app_handle: AppHandle,
    _code: String,
    _state: String,
) -> Result<QQLoginResult, String> {
    Err("This method is deprecated, please use the new simplified mode".to_string())
}