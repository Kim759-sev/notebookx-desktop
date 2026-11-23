# NotebookX Desktop

<p align="center">
  <strong>你的专属云端桌面版 ComfyUI · Google Colab 算力管家 · 费用看门狗 · 本地自动唤醒工作流网关</strong>
</p>

`NotebookX Desktop` 是一款基于 **Rust + Tauri v2 + React 19** 构建的原生桌面工作站（Windows 原生直连免装 WSL，同时支持 macOS / Linux）。它将 Google Colab 的高性能 GPU（如 **G4 RTX PRO 6000 96GB**、**A100 80GB**、**H100**、**L4**、**T4**）与你的 **Google Drive（如 30TB 模型与工作流库）** 无缝连接成**点击即用、关闭即停扣费**的专属云端桌面版 ComfyUI。

---

## 核心特性

- **云端桌面版 ComfyUI 一键直连**
  - 一键完成：**分配目标显卡（支持最低显存校验，抽不到 80G A100 自动退卡重试）→ 挂载 Google Drive → 秒级恢复自定义节点与模型映射 → 打通本地 `127.0.0.1:8188` SSH 隧道 → 出片级节点深度自检（如 `H3C_Timeline` / `H3C_SaveSegment`）**。
  - 点击 **「打开桌面版 ComfyUI 窗口」** 直接弹出独立原生窗口操作，告别每次复制粘贴临时 `trycloudflare.com` 隧道地址。
  - 支持与本机 **DramaSmith (`127.0.0.1:8018`)** 自动同步连接配置。

- **配置永久保存 & 工作流缺模型体检 + CPU 零积分预下载**
  - **配置/插件快照固化**：在 ComfyUI 中装好的 Custom Nodes、界面设置与工作流一键快照保存到 Google Drive，下次开机十几秒直接解压到云端高速 NVMe 盘恢复。
  - **缺模型扫描器**：导入任意 ComfyUI `.json` 工作流，自动对比云盘已有模型，精准列出缺失的大模型 / LoRA / VAE / 文本编码器。
  - **省积分「CPU 备货模式」**：无需开启昂贵的 G4/A100 显卡，一键拉起 0 显卡积分的 CPU 实例挂载云盘，利用 Google 机房千兆带宽把缺失模型一次性下载进你的 Google Drive，下完自动关机。

- **多工作流档位自由切换**
  - 内置 `DramaSmith 文武 H3 · BF16 连续性出片 (G4 96GB)`、`Flux / SDXL 高清生图 (L4 24GB)`、`Wan / Hunyuan 视频工作流 (A100 80GB)` 与 `CPU 零积分预下模型`，并支持自定义新建任意工作流配置档。

- **Serverless 本地常驻 API 网关（`127.0.0.1:18188`）**
  - 支持外部程序直接调用 `POST /api/v1/comfy/run`（或透明代理原生 ComfyUI `/prompt` 接口）出视频/出图（原生支持 `start` 首段与 `continue` + `parentTaskId` 多镜头连续性参数）。
  - **按需自动冷启动**：若收到 API 请求时云端未开机，自动拉起绑定的 GPU 档位 → 挂盘启动 ComfyUI → 执行工作流返回结果 → 闲置超时后自动安全备份并关机停止扣费。

- **双重费用保护 & 100% 确认停机**
  - **云端独立 `watchdog.py`**：直接运行在 Colab 虚拟机内，即使关掉本机桌面软件或电脑断电，定时停止与闲置停止（默认 30 分钟，且实时检测 ComfyUI `/queue` 绝不误杀渲染中的视频任务）依然照常生效。
  - **100% 确认停止扣费**：停机前先完成成片与续接序列缓存备份，调用 `unassign` 后二次向 Google 后端核验实例彻底消失，杜绝“隧道断了但 GPU 还在后台偷偷扣费”。

- **多 Google 账号管理 & 一键部署 GGUF 大模型插件**
  - 同时管理多个 Google 账号（独立 `--config` 状态隔离），实时查看每个账号的 `CO PRO+` / `PRO` 积分余额、消耗速率与剩余可用时长。
  - 内置 **Hugging Face GGUF / `llama.cpp` 插件**：输入仓库名即可按实际分到的显存自动挑选放得下的最佳量化精度，一键提供本地 OpenAI 兼容 API（`/v1/chat/completions`）与内置网页聊天。
  - 支持 **简体中文 / 繁體中文 / English** 与 **浅色 / 深色 / 跟随系统** 主题。

---

## 本地开发与构建

### 环境要求
- Windows 10/11（原生自带 OpenSSH，无需安装 WSL）、macOS 或 Linux
- Rust 1.80+ (`cargo`)
- Node.js 20+ (`npm`)
- Python 3.10+ 与 `uv`（软件内置一键安装 `google-colab-cli` 引导）

### 启动开发模式
```powershell
npm.cmd install
npm.cmd run tauri dev
```

### 构建桌面安装包 / 可执行程序
```powershell
npm.cmd run tauri build
```

---

## 本地 API 网关快速调用示例

```bash
# 查看网关与云端实例实时状态
curl http://127.0.0.1:18188/api/v1/status

# 调用工作流生成视频/图片（若云端未开机将自动拉起并挂载 Google Drive）
curl -X POST http://127.0.0.1:18188/api/v1/comfy/run \
  -H "Content-Type: application/json" \
  -d '{
    "workflowId": "colab-bf16-continuity",
    "continuityMode": "start",
    "sequenceName": "ep01_shot03",
    "params": {
      "prompt": "电影级武侠雨夜对决，刀光水花特写，24fps",
      "frames": 243,
      "seed": 20261123
    },
    "autoWake": true,
    "waitForCompletion": true
  }'
```

## License

MIT
