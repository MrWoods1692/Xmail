const express = require('express');
const session = require('express-session');
const cors = require('cors');
const bodyParser = require('body-parser');
require('dotenv').config();

const Database = require('./config/database');
const Admin = require('./models/Admin');
const authRoutes = require('./routes/auth');
const dashboardRoutes = require('./routes/dashboard');

const app = express();
const PORT = process.env.PORT || 1457;

// 中间件配置
app.use(cors({
  origin: true,
  credentials: true
}));

app.use(bodyParser.json());
app.use(bodyParser.urlencoded({ extended: true }));

// 静态文件服务
app.use('/public', express.static('public'));

// Session 配置
app.use(session({
  secret: process.env.SESSION_SECRET,
  resave: false,
  saveUninitialized: false,
  cookie: {
    secure: false, // 开发环境设为 false
    maxAge: 24 * 60 * 60 * 1000 // 24小时
  }
}));

// 初始化数据库和模型
async function initializeApp() {
  try {
    console.log('正在初始化数据库连接...');
    const database = new Database();
    
    // 初始化管理员表
    await database.initAdminTable();
    
    // 创建管理员模型实例
    const adminModel = new Admin(database);
    
    // 初始化默认管理员
    await adminModel.initDefaultAdmin();
    
    // 将模型实例添加到 app.locals 以便在路由中使用
    app.locals.Database = database;
    app.locals.Admin = adminModel;
    
    console.log('数据库初始化完成');
  } catch (error) {
    console.error('数据库初始化失败:', error);
    process.exit(1);
  }
}

// 路由配置
app.use('/', authRoutes);
app.use('/', dashboardRoutes);

// 根路径重定向到仪表板
app.get('/', (req, res) => {
  if (req.session && req.session.admin) {
    res.redirect('/dashboard');
  } else {
    res.redirect('/login');
  }
});

// 404 处理
app.use((req, res) => {
  res.status(404).send(`
    <!DOCTYPE html>
    <html lang="zh-CN">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>页面未找到 - ${process.env.APP_NAME}</title>
        <link rel="icon" type="image/x-icon" href="/public/favicon.ico">
        <link rel="stylesheet" href="/public/css/fonts.css">
        <style>
            body {
                background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                min-height: 100vh;
                display: flex;
                align-items: center;
                justify-content: center;
                margin: 0;
            }
            .error-container {
                background: white;
                padding: 3rem;
                border-radius: 12px;
                box-shadow: 0 10px 30px rgba(0, 0, 0, 0.2);
                text-align: center;
                max-width: 500px;
            }
            .error-code {
                font-size: 4rem;
                font-weight: 700;
                color: #667eea;
                margin-bottom: 1rem;
            }
            .error-message {
                font-size: 1.2rem;
                color: #333;
                margin-bottom: 2rem;
            }
            .back-btn {
                padding: 0.75rem 2rem;
                background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                color: white;
                text-decoration: none;
                border-radius: 8px;
                font-weight: 600;
                transition: transform 0.2s;
            }
            .back-btn:hover {
                transform: translateY(-2px);
            }
        </style>
    </head>
    <body>
        <div class="error-container">
            <div class="error-code">404</div>
            <div class="error-message">页面未找到</div>
            <a href="/" class="back-btn">返回首页</a>
        </div>
    </body>
    </html>
  `);
});

// 错误处理
app.use((error, req, res, next) => {
  console.error('服务器错误:', error);
  res.status(500).json({
    success: false,
    message: '服务器内部错误'
  });
});

// 启动服务器
async function startServer() {
  await initializeApp();
  
  app.listen(PORT, () => {
    console.log(`\n🚀 ${process.env.APP_NAME} 启动成功!`);
    console.log(`📍 服务地址: http://localhost:${PORT}`);
    console.log(`👤 管理员账号: ${process.env.ADMIN_USERNAME}`);
    console.log(`🔑 管理员密码: ${process.env.ADMIN_PASSWORD}`);
    console.log(`📊 数据库: ${process.env.DB_NAME}`);
    console.log('─'.repeat(50));
  });
}

// 优雅关闭
process.on('SIGINT', async () => {
  console.log('\n正在关闭服务器...');
  if (app.locals.Database) {
    await app.locals.Database.close();
  }
  process.exit(0);
});

startServer().catch(console.error);
