@echo off
chcp 65001 >nul
cd /d "%~dp0"

if exist "%~dp0bin\notebookx-desktop.exe" (
    start "" "%~dp0bin\notebookx-desktop.exe"
    exit /b 0
)

if exist "%~dp0src-tauri\target\release\notebookx-desktop.exe" (
    if not exist "%~dp0bin" mkdir "%~dp0bin"
    copy /y "%~dp0src-tauri\target\release\notebookx-desktop.exe" "%~dp0bin\notebookx-desktop.exe" >nul
    start "" "%~dp0bin\notebookx-desktop.exe"
    exit /b 0
)

echo [NotebookX] 首次运行未检测到独立可执行文件，正在构建精简版 Release 桌面端...
call npx.cmd tauri build --no-bundle
if errorlevel 1 (
    echo [NotebookX] 构建失败，请检查终端输出。
    pause
    exit /b 1
)
if not exist "%~dp0bin" mkdir "%~dp0bin"
copy /y "%~dp0src-tauri\target\release\notebookx-desktop.exe" "%~dp0bin\notebookx-desktop.exe" >nul
start "" "%~dp0bin\notebookx-desktop.exe"
