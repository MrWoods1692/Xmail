@echo off
echo ========================================
echo        XMail 开发环境启动脚本
echo ========================================
echo.
echo 请选择要启动的服务:
echo 1. 启动主应用 (Tauri)
echo 2. 启动后台管理 (Node.js)
echo 3. 同时启动两个服务
echo 4. 退出
echo.
set /p choice=请输入选项 (1-4):

if "%choice%"=="1" goto start_main
if "%choice%"=="2" goto start_admin
if "%choice%"=="3" goto start_both
if "%choice%"=="4" goto exit
echo 无效选项，请重新运行脚本
pause
goto exit

:start_main
echo.
echo ========================================
echo 正在启动 XMail 主应用...
echo ========================================
cd /d "tauri-app"
cargo tauri dev
goto exit

:start_admin
echo.
echo ========================================
echo 正在启动 XMail 后台管理...
echo ========================================
cd /d "tauri-app/admin-backend"
if not exist "node_modules" (
    echo 正在安装依赖...
    npm install
)
echo 访问地址: http://localhost:1457
echo 管理员账号: zhangying
echo 管理员密码: zhangying
echo.
npm run dev
goto exit

:start_both
echo.
echo ========================================
echo 正在同时启动两个服务...
echo ========================================
echo.
echo 1. 启动后台管理服务 (端口 1457)
cd /d "tauri-app/admin-backend"
if not exist "node_modules" (
    echo 正在安装后台管理依赖...
    npm install
)
start "XMail 后台管理" cmd /k "npm run dev"
echo 后台管理已启动: http://localhost:1457
echo.

echo 2. 启动主应用服务
cd /d "../"
echo 正在启动 Tauri 主应用...
cargo tauri dev
goto exit

:exit
