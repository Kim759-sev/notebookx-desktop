@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo [NotebookX] 正在检查本地 Vite 热更新服务 (http://localhost:1420)...
netstat -ano | findstr ":1420 " | findstr "LISTENING" >nul
if errorlevel 1 (
    echo [NotebookX] 正在启动前端热更新服务 Vite HMR...
    start "NotebookX Vite HMR" cmd /k "cd /d ""%~dp0"" && npm.cmd run dev"
    timeout /t 2 /nobreak >nul
)
echo [NotebookX] 正在启动热更新桌面端 (修改 src 代码实时刷新界面)...
start "" "%~dp0src-tauri\target\debug\notebookx-desktop.exe"
