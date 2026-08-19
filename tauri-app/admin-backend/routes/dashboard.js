const express = require('express');
const router = express.Router();
const { requireAuthPage } = require('../middleware/auth');

// 仪表板页面
router.get('/dashboard', requireAuthPage, (req, res) => {
  const admin = req.session.admin;
  
  res.send(`
    <!DOCTYPE html>
    <html lang="zh-CN">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>${process.env.APP_NAME}</title>
        <link rel="icon" type="image/x-icon" href="/public/favicon.ico">
        <link rel="stylesheet" href="/public/css/fonts.css">
        <style>
            * {
                margin: 0;
                padding: 0;
                box-sizing: border-box;
            }
            
            body {
                background: #f5f5f5;
                min-height: 100vh;
            }
            
            .header {
                background: white;
                box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
                padding: 0 2rem;
                height: 60px;
                display: flex;
                align-items: center;
                justify-content: space-between;
                position: fixed;
                top: 0;
                left: 0;
                right: 0;
                z-index: 1000;
            }
            
            .header-left {
                display: flex;
                align-items: center;
                gap: 1rem;
            }
            
            .logo {
                font-size: 1.5rem;
                font-weight: 700;
                background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                -webkit-background-clip: text;
                -webkit-text-fill-color: transparent;
                background-clip: text;
            }
            
            .header-right {
                display: flex;
                align-items: center;
                gap: 1rem;
            }
            
            .user-info {
                color: #666;
                font-size: 0.9rem;
            }
            
            .logout-btn {
                padding: 0.5rem 1rem;
                background: #dc3545;
                color: white;
                border: none;
                border-radius: 6px;
                cursor: pointer;
                font-size: 0.9rem;
                transition: background 0.3s;
            }
            
            .logout-btn:hover {
                background: #c82333;
            }
            
            .main-container {
                display: flex;
                margin-top: 60px;
                min-height: calc(100vh - 60px);
            }
            
            .sidebar {
                width: 250px;
                background: white;
                box-shadow: 2px 0 4px rgba(0, 0, 0, 0.1);
                padding: 2rem 0;
            }
            
            .sidebar-menu {
                list-style: none;
            }
            
            .sidebar-menu li {
                margin-bottom: 0.5rem;
            }
            
            .sidebar-menu a {
                display: block;
                padding: 0.75rem 2rem;
                color: #333;
                text-decoration: none;
                transition: all 0.3s ease;
                cursor: pointer;
            }

            .sidebar-menu a:hover,
            .sidebar-menu a.active {
                background: #f8f9fa;
                border-right: 3px solid #ff6b6b;
                color: #ff6b6b;
            }
            
            .content {
                flex: 1;
                padding: 2rem;
            }
            
            .page-header {
                margin-bottom: 2rem;
            }
            
            .page-title {
                font-size: 2rem;
                color: #333;
                margin-bottom: 0.5rem;
            }
            
            .page-subtitle {
                color: #666;
                font-size: 1rem;
            }
            
            .content-card {
                background: white;
                border-radius: 12px;
                padding: 2rem;
                box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
            }
            
            .welcome-message {
                text-align: center;
                padding: 3rem 2rem;
                color: #666;
            }
            
            .welcome-message h3 {
                color: #333;
                margin-bottom: 1rem;
                font-size: 1.5rem;
            }
            
            .welcome-message p {
                font-size: 1rem;
                line-height: 1.6;
            }

            /* 页面内容切换样式 */
            .page-content {
                display: none;
                opacity: 0;
                transition: opacity 0.3s ease;
            }

            .page-content.active {
                display: block;
                opacity: 1;
            }

            .placeholder-message {
                text-align: center;
                padding: 3rem 2rem;
                color: #666;
            }

            .placeholder-message h3 {
                color: #333;
                margin-bottom: 1rem;
                font-size: 1.5rem;
            }

            .placeholder-message p {
                font-size: 1rem;
                line-height: 1.6;
            }
        </style>
    </head>
    <body>
        <header class="header">
            <div class="header-left">
                <div class="logo">${process.env.APP_NAME}</div>
            </div>
            <div class="header-right">
                <span class="user-info">欢迎，${admin.username}</span>
                <button class="logout-btn" onclick="logout()">退出登录</button>
            </div>
        </header>
        
        <div class="main-container">
            <aside class="sidebar">
                <ul class="sidebar-menu">
                    <li><a onclick="showPage('dashboard')" class="menu-item active" data-page="dashboard">仪表板</a></li>
                    <li><a onclick="showPage('users')" class="menu-item" data-page="users">用户管理</a></li>
                </ul>
            </aside>

            <main class="content">
                <!-- 仪表板内容 -->
                <div id="dashboard-content" class="page-content active">
                    <div class="page-header">
                        <h1 class="page-title">仪表板</h1>
                        <p class="page-subtitle">欢迎使用 ${process.env.APP_NAME}</p>
                    </div>

                    <div class="content-card">
                        <div class="welcome-message">
                            <h3>欢迎使用 XMail 管理后台</h3>
                            <p>您可以通过左侧菜单访问各种管理功能。<br>
                            当前登录用户：<strong>${admin.username}</strong><br>
                            登录时间：<strong>${new Date().toLocaleString('zh-CN')}</strong></p>
                        </div>
                    </div>
                </div>

                <!-- 用户管理内容 -->
                <div id="users-content" class="page-content">
                    <div class="page-header">
                        <h1 class="page-title">用户管理</h1>
                        <p class="page-subtitle">管理系统用户</p>
                    </div>

                    <div class="content-card">
                        <div class="placeholder-message">
                            <h3>用户管理功能</h3>
                            <p>此功能正在开发中，请等待后续更新。<br>
                            您可以在这里管理系统的用户账户、权限等信息。</p>
                        </div>
                    </div>
                </div>
            </main>
        </div>
        
        <script>
            // 页面切换函数
            function showPage(pageId) {
                // 隐藏所有页面内容
                const allPages = document.querySelectorAll('.page-content');
                allPages.forEach(page => {
                    page.classList.remove('active');
                });

                // 显示指定页面内容
                const targetPage = document.getElementById(pageId + '-content');
                if (targetPage) {
                    targetPage.classList.add('active');
                }

                // 更新菜单激活状态
                const allMenuItems = document.querySelectorAll('.menu-item');
                allMenuItems.forEach(item => {
                    item.classList.remove('active');
                });

                const activeMenuItem = document.querySelector(\`[data-page="\${pageId}"]\`);
                if (activeMenuItem) {
                    activeMenuItem.classList.add('active');
                }
            }

            // 退出登录函数
            async function logout() {
                if (confirm('确定要退出登录吗？')) {
                    try {
                        const response = await fetch('/logout', {
                            method: 'POST',
                            headers: {
                                'Content-Type': 'application/json',
                            }
                        });

                        const result = await response.json();

                        if (result.success) {
                            window.location.href = '/login';
                        } else {
                            alert('退出登录失败');
                        }
                    } catch (error) {
                        console.error('退出登录错误:', error);
                        alert('退出登录失败');
                    }
                }
            }
        </script>
    </body>
    </html>
  `);
});

// 用户管理页面重定向到仪表板
router.get('/users', requireAuthPage, (req, res) => {
  res.redirect('/dashboard');
});

module.exports = router;
