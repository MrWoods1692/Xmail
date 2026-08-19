# XMail - 跨平台邮箱客户端

基于 Tauri 2 + Svelte + TypeScript + Rust 构建的现代化跨平台邮箱客户端。

## 功能特性

- 🌐 **跨平台支持**: Windows、macOS、Linux
- 📧 **多账户管理**: 支持添加多个邮箱账户
- 🔒 **安全连接**: TLS加密邮件传输
- 🎨 **现代化界面**: 基于Svelte的响应式UI设计
- ⚡ **高性能**: Rust后端 + 原生性能
- 📱 **直观操作**: 类似现代邮箱客户端的用户体验

## 技术栈

- **前端**: Svelte + TypeScript + Vite
- **后端**: Rust + Tauri 2
- **数据库**: SQLite (计划中)
- **邮件协议**: IMAP/SMTP (计划中)

## 开发环境

### 必需工具
- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/)
- [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) (Windows)

### 推荐IDE
- [VS Code](https://code.visualstudio.com/)
- [Svelte扩展](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode)
- [Tauri扩展](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## 快速开始

1. **安装依赖**
   ```bash
   npm install
   ```

2. **启动开发服务器**
   ```bash
   # Windows (需要Visual Studio环境)
   .\build_tauri.bat

   # macOS/Linux
   npm run tauri dev
   ```

3. **构建应用**
   ```bash
   npm run tauri build
   ```

## 项目状态

🚧 **开发中** - 当前版本为基础框架，邮件功能正在开发中。

### 已完成
- ✅ 项目架构搭建
- ✅ 基础UI组件
- ✅ 账户管理界面
- ✅ 邮件列表界面
- ✅ 邮件阅读器界面

### 计划中
- 🔄 IMAP/SMTP邮件功能
- 🔄 本地数据存储
- 🔄 邮件搜索和过滤
- 🔄 附件处理
- 🔄 OAuth认证支持

## 许可证

MIT License
