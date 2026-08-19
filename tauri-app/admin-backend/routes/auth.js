const express = require('express');
const router = express.Router();
const { requireGuest } = require('../middleware/auth');

// 登录页面
router.get('/login', requireGuest, (req, res) => {
  res.send(`
    <!DOCTYPE html>
    <html lang="zh-CN">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>登录 - ${process.env.APP_NAME}</title>
        <link rel="icon" type="image/x-icon" href="/public/favicon.ico">
        <link rel="stylesheet" href="/public/css/fonts.css">
        <style>
            * {
                margin: 0;
                padding: 0;
                box-sizing: border-box;
            }

            body {
                background: linear-gradient(135deg, #ff9a9e 0%, #fecfef 50%, #fecfef 100%);
                min-height: 100vh;
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 2rem;
                position: relative;
                overflow: hidden;
            }

            /* 朦胧气泡装饰 */
            .bubble-decoration {
                position: absolute;
                width: 100%;
                height: 100%;
                pointer-events: none;
                z-index: 0;
            }

            .bubble {
                position: absolute;
                border-radius: 50%;
                background: rgba(255, 255, 255, 0.1);
                backdrop-filter: blur(10px);
                border: 1px solid rgba(255, 255, 255, 0.2);
            }

            .bubble-1 {
                width: 150px;
                height: 150px;
                top: 10%;
                left: 5%;
                background: rgba(255, 182, 193, 0.15);
            }

            .bubble-2 {
                width: 100px;
                height: 100px;
                top: 15%;
                right: 10%;
                background: rgba(255, 218, 185, 0.12);
            }

            .bubble-3 {
                width: 200px;
                height: 200px;
                bottom: 20%;
                left: 8%;
                background: rgba(255, 192, 203, 0.1);
            }

            .bubble-4 {
                width: 80px;
                height: 80px;
                bottom: 30%;
                right: 15%;
                background: rgba(255, 228, 225, 0.18);
            }

            .bubble-5 {
                width: 120px;
                height: 120px;
                top: 50%;
                left: 2%;
                background: rgba(255, 240, 245, 0.15);
            }

            .bubble-6 {
                width: 90px;
                height: 90px;
                top: 70%;
                right: 5%;
                background: rgba(255, 182, 193, 0.12);
            }
            
            .login-container {
                background: rgba(255, 255, 255, 0.95);
                backdrop-filter: blur(20px);
                border-radius: 20px;
                box-shadow:
                    0 20px 40px rgba(0, 0, 0, 0.1),
                    0 0 0 1px rgba(255, 255, 255, 0.2);
                width: 100%;
                max-width: 900px;
                min-height: 500px;
                display: flex;
                overflow: hidden;
                border: 1px solid rgba(255, 255, 255, 0.3);
            }

            .login-left {
                flex: 1;
                background: linear-gradient(135deg, #ff6b6b 0%, #ff8e8e 50%, #ffa8a8 100%);
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 3rem;
                position: relative;
            }

            .welcome-image {
                max-width: 100%;
                max-height: 300px;
                object-fit: contain;
                filter: drop-shadow(0 10px 30px rgba(0, 0, 0, 0.3));
            }

            .login-right {
                flex: 1;
                padding: 3rem 2.5rem;
                display: flex;
                flex-direction: column;
                justify-content: center;
            }
            
            .login-header {
                text-align: center;
                margin-bottom: 2.5rem;
            }

            .login-icon {
                width: 60px;
                height: 60px;
                margin: 0 auto 1.5rem;
                display: block;
                object-fit: contain;
            }

            .login-header h1 {
                background: linear-gradient(135deg, #ff6b6b 0%, #ff8e8e 100%);
                -webkit-background-clip: text;
                -webkit-text-fill-color: transparent;
                background-clip: text;
                margin-bottom: 0.8rem;
                font-size: 2rem;
                font-weight: 700;
                letter-spacing: -0.5px;
            }

            .login-header p {
                color: #e91e63;
                font-size: 1rem;
                opacity: 0.9;
            }
            
            .form-group {
                margin-bottom: 2rem;
                position: relative;
            }

            .form-group label {
                display: block;
                margin-bottom: 0.8rem;
                color: #d63384;
                font-weight: 600;
                font-size: 0.95rem;
                letter-spacing: 0.3px;
            }

            .form-group input {
                width: 100%;
                padding: 1rem 1.2rem;
                border: 2px solid rgba(255, 107, 107, 0.2);
                border-radius: 12px;
                font-size: 1rem;
                background: rgba(255, 255, 255, 0.9);
                transition: all 0.3s ease;
            }

            .form-group input:focus {
                outline: none;
                border-color: #ff6b6b;
                background: rgba(255, 255, 255, 1);
                box-shadow: 0 0 0 4px rgba(255, 107, 107, 0.1);
            }

            .form-group input::placeholder {
                color: #ff9a9e;
                opacity: 0.8;
            }
            
            .login-btn {
                width: 100%;
                padding: 1rem 1.5rem;
                background: linear-gradient(135deg, #ff6b6b 0%, #ff8e8e 100%);
                color: white;
                border: none;
                border-radius: 12px;
                font-size: 1.1rem;
                font-weight: 600;
                cursor: pointer;
                transition: all 0.3s ease;
                letter-spacing: 0.5px;
                margin-top: 1rem;
            }

            .login-btn:hover {
                transform: translateY(-2px);
                box-shadow: 0 8px 20px rgba(255, 107, 107, 0.4);
            }

            .login-btn:active {
                transform: translateY(0);
            }
            
            .error-message {
                background: linear-gradient(135deg, #fee2e2, #fecaca);
                color: #dc2626;
                padding: 1rem 1.2rem;
                border-radius: 12px;
                margin-bottom: 1.5rem;
                border: 1px solid rgba(220, 38, 38, 0.2);
                font-weight: 500;
                box-shadow: 0 4px 15px rgba(220, 38, 38, 0.1);
            }

            /* 响应式设计 */
            @media (max-width: 768px) {
                .login-container {
                    flex-direction: column;
                    max-width: 400px;
                    min-height: auto;
                }

                .login-left {
                    padding: 2rem;
                }

                .welcome-image {
                    max-height: 200px;
                }

                .login-right {
                    padding: 2rem 1.5rem;
                }
            }
        </style>
    </head>
    <body>
        <!-- 朦胧气泡装饰 -->
        <div class="bubble-decoration">
            <div class="bubble bubble-1"></div>
            <div class="bubble bubble-2"></div>
            <div class="bubble bubble-3"></div>
            <div class="bubble bubble-4"></div>
            <div class="bubble bubble-5"></div>
            <div class="bubble bubble-6"></div>
        </div>

        <div class="login-container">
            <!-- 左侧欢迎图片 -->
            <div class="login-left">
                <img src="/public/image/huanying.png" alt="欢迎使用XMail" class="welcome-image">
            </div>

            <!-- 右侧登录表单 -->
            <div class="login-right">
                <div class="login-header">
                    <img src="/public/image/haokan.png" alt="登录图标" class="login-icon">
                    <h1>${process.env.APP_NAME}</h1>
                    <p>请登录以继续管理</p>
                </div>

                <form id="loginForm" method="POST" action="/login">
                    <div class="form-group">
                        <label for="username">👤 用户名</label>
                        <input type="text" id="username" name="username" placeholder="请输入用户名" required>
                    </div>

                    <div class="form-group">
                        <label for="password">🔒 密码</label>
                        <input type="password" id="password" name="password" placeholder="请输入密码" required>
                    </div>

                    <button type="submit" class="login-btn">🚀 立即登录</button>
                </form>
            </div>
        </div>
        
        <script>
            document.getElementById('loginForm').addEventListener('submit', async (e) => {
                e.preventDefault();
                
                const formData = new FormData(e.target);
                const data = {
                    username: formData.get('username'),
                    password: formData.get('password')
                };
                
                try {
                    const response = await fetch('/login', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                        },
                        body: JSON.stringify(data)
                    });
                    
                    const result = await response.json();
                    
                    if (result.success) {
                        window.location.href = '/dashboard';
                    } else {
                        // 显示错误信息
                        let errorDiv = document.querySelector('.error-message');
                        if (!errorDiv) {
                            errorDiv = document.createElement('div');
                            errorDiv.className = 'error-message';
                            document.querySelector('form').insertBefore(errorDiv, document.querySelector('.form-group'));
                        }
                        errorDiv.textContent = result.message;
                    }
                } catch (error) {
                    console.error('登录错误:', error);
                    alert('登录失败，请重试');
                }
            });
        </script>
    </body>
    </html>
  `);
});

// 登录处理
router.post('/login', async (req, res) => {
  const { username, password } = req.body;
  
  if (!username || !password) {
    return res.status(400).json({
      success: false,
      message: '用户名和密码不能为空'
    });
  }
  
  try {
    const Admin = req.app.locals.Admin;
    const result = await Admin.login(username, password);
    
    if (result.success) {
      req.session.admin = result.admin;
      res.json({
        success: true,
        message: '登录成功',
        redirect: '/dashboard'
      });
    } else {
      res.status(401).json(result);
    }
  } catch (error) {
    console.error('登录错误:', error);
    res.status(500).json({
      success: false,
      message: '服务器错误'
    });
  }
});

// 退出登录
router.post('/logout', (req, res) => {
  req.session.destroy((err) => {
    if (err) {
      return res.status(500).json({
        success: false,
        message: '退出登录失败'
      });
    }
    res.json({
      success: true,
      message: '退出登录成功',
      redirect: '/login'
    });
  });
});

module.exports = router;
