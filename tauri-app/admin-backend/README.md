# XMail 管理后台

XMail 邮件系统的后台管理界面，基于 Node.js + Express 构建。

## 功能特性

- 🔐 安全的登录认证系统
- 👤 用户管理功能（开发中）
- 📊 仪表板概览
- 🎨 现代化的响应式界面
- 🔒 Session 会话管理

## 技术栈

- **后端框架**: Express.js
- **数据库**: MySQL
- **认证**: bcryptjs + express-session
- **前端**: 原生 HTML/CSS/JavaScript
- **环境配置**: dotenv

## 快速开始

### 1. 安装依赖

```bash
cd admin-backend
npm install
```

### 2. 配置环境变量

复制 `.env` 文件并根据需要修改配置：

```bash
# 服务器配置
PORT=1457
NODE_ENV=development

# 数据库配置
DB_HOST=localhost
DB_PORT=3306
DB_USER=root
DB_PASSWORD=
DB_NAME=xmail

# Session 配置
SESSION_SECRET=xmail_admin_secret_key_2024

# 管理员配置
ADMIN_USERNAME=zhangying
ADMIN_PASSWORD=zhangying

# 应用配置
APP_NAME=XMail管理后台
```

### 3. 启动服务

```bash
# 开发模式（自动重启）
npm run dev

# 生产模式
npm start
```

### 4. 访问系统

打开浏览器访问: http://localhost:1457

默认管理员账号:
- 用户名: zhangying
- 密码: zhangying

## 项目结构

```
admin-backend/
├── config/
│   └── database.js          # 数据库配置
├── middleware/
│   └── auth.js              # 认证中间件
├── models/
│   └── Admin.js             # 管理员模型
├── routes/
│   ├── auth.js              # 认证路由
│   └── dashboard.js         # 仪表板路由
├── .env                     # 环境变量配置
├── package.json             # 项目依赖
├── server.js                # 主服务器文件
└── README.md                # 项目说明
```

## API 接口

### 认证相关

- `GET /login` - 登录页面
- `POST /api/auth/login` - 登录处理
- `POST /api/auth/logout` - 退出登录

### 页面路由

- `GET /` - 首页（重定向）
- `GET /dashboard` - 仪表板
- `GET /users` - 用户管理

## 安全特性

- 密码使用 bcryptjs 加密存储
- Session 会话管理
- 路由权限验证
- SQL 注入防护
- XSS 防护

## 开发说明

- 管理员账号密码存储在数据库中，通过环境变量配置
- 系统启动时会自动创建默认管理员账号
- 所有敏感配置都通过环境变量管理
- 支持开发和生产环境配置

## 注意事项

1. 请确保 MySQL 数据库已启动并可连接
2. 首次运行会自动创建必要的数据表
3. 生产环境请修改默认的 SESSION_SECRET
4. 建议在生产环境中使用 HTTPS
