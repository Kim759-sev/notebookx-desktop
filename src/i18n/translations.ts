import type { LocaleCode } from '../lib/types';

// The English label is also the lookup key. Data supplied by users is displayed verbatim.
const catalog = `
Workspace|工作空间|工作空間
System|系统|系統
Cloud studio|云端工作台|雲端工作台
Model library|模型库|模型庫
Workflow API|工作流 API|工作流 API
Accounts & sessions|账号与会话|帳號與工作階段
Models & plugins|大模型与插件|大型模型與外掛
Environment & settings|环境与设置|環境與設定
Cloud ComfyUI Studio|云端 ComfyUI 工作台|雲端 ComfyUI 工作台
Launch a workflow, connect to ComfyUI, and manage your cloud runtime.|选择工作流配置，启动并管理你的云端创作环境。|選擇工作流設定，啟動並管理你的雲端創作環境。
Live Colab|实盘 Colab|正式 Colab
Mock sandbox|演示沙盒 Mock|示範沙盒 Mock
Light|浅色|淺色
Dark|深色|深色
System theme|跟随系统|跟隨系統
Balance|余额|餘額
Burn|消耗|消耗
Remaining|预计续航|預估續航
Online|在线|線上
Offline|离线|離線
Refresh|刷新|重新整理
No account|未登录|未登入
Local gateway|本地 API 网关|本機 API 閘道
Desktop workstation|桌面工作站|桌面工作站
Menu|菜单|選單
Theme|主题|主題
Language|语言|語言
Backend mode|后端模式|後端模式
Ready to launch|就绪，等待启动|就緒，等待啟動
Standby|待机|待命
Runtime|计算实例|運算執行個體
Selected profile|当前配置|目前設定
Launch cloud ComfyUI|一键启动云端 ComfyUI|一鍵啟動雲端 ComfyUI
Launch CPU runtime|启动 CPU 下载实例|啟動 CPU 下載執行個體
Workflow profiles|多工作流配置档|多工作流設定檔
New profile|新建配置档|新增設定檔
Edit profile|编辑配置档|編輯設定檔
Profile name|配置名称|設定檔名稱
Description|描述|說明
Accelerator|加速器|加速器
Minimum VRAM (GB)|最低显存 (GB)|最低顯示記憶體 (GB)
VRAM retries|显存不符重试次数|顯示記憶體不足重試次數
High-RAM|高内存|高記憶體
Drive root|云盘根目录|雲端硬碟根目錄
Drive mount|云盘挂载方式|雲端硬碟掛載方式
Native mount|原生挂载|原生掛載
Unattended rclone|无人值守 rclone|無人值守 rclone
Disabled|禁用|停用
Startup mode|启动方式|啟動方式
Managed script|托管脚本|託管指令碼
ComfyUI snapshot|ComfyUI 快照|ComfyUI 快照
CPU pre-download|CPU 预下载|CPU 預先下載
Script path|启动脚本相对路径|啟動指令碼相對路徑
Required nodes|必需自定义节点|必要自訂節點
Cost guards|扣费保护|扣費防護
Scheduled stop (min)|定时停止（分钟）|定時停止（分鐘）
Idle stop (min)|闲置停止（分钟）|閒置停止（分鐘）
Spending cap (CCU)|消费上限（CCU）|消費上限（CCU）
Balance floor (CCU)|最低余额（CCU）|最低餘額（CCU）
Back up before shutdown|停止前备份|停止前備份
Save|保存|儲存
Cancel|取消|取消
Delete|删除|刪除
Close|关闭|關閉
Edit|编辑|編輯
Confirm|确认|確認
Retry|重试|重試
Selected|已选中|已選取
Select|选择|選取
Active|使用中|使用中
Idle stop|闲置关机|閒置關機
min|分钟|分鐘
retries|次重试|次重試
Cinematic video generation|电影级视频生成|電影級影片生成
Image generation|图像生成|影像生成
Video generation|视频生成|影片生成
Zero-GPU model downloads|零积分模型预下载|零點數模型預先下載
CPU Pre-download|CPU 零积分预下载|CPU 零點數預先下載
DramaSmith H3 · BF16|DramaSmith 文武 H3 · BF16|DramaSmith 文武 H3 · BF16
Runtime connections|运行环境连接|執行環境連線
Connection|连接服务|連線服務
Endpoint|服务地址|服務位址
Status|状态|狀態
Management|管理面板|管理面板
Awaiting runtime|等待实例启动|等待執行個體啟動
View sessions|查看会话|檢視工作階段
Runtime pipeline|实时启动流程|即時啟動流程
Allocate GPU and verify VRAM|分配显卡与显存核验|分配顯示卡與顯示記憶體核驗
Mount Google Drive|挂载 30TB Google Drive|掛載 30TB Google Drive
Restore nodes and models|恢复插件与模型配置|還原外掛與模型設定
Connect local tunnel|本地 127.0.0.1 隧道直连|本機 127.0.0.1 通道直連
Check nodes and queue|出片级自检 H3C / 队列就绪|輸出自檢 H3C / 佇列就緒
Authorize Drive|授权 Google Drive|授權 Google Drive
Open desktop ComfyUI|打开桌面版 ComfyUI 窗口|開啟桌面版 ComfyUI 視窗
Open in browser|外部浏览器打开|在外部瀏覽器開啟
Save environment snapshot|保存当前配置/插件快照到云盘|儲存目前設定與外掛快照至雲端硬碟
Snapshots preserve custom nodes and settings in Google Drive.|快照将自定义节点和设置长期保存在 Google Drive。|快照將自訂節點與設定長期儲存在 Google Drive。
Sync endpoint to DramaSmith|一键同步地址到 DramaSmith|一鍵同步位址至 DramaSmith
Back up and stop billing|安全备份并停止扣费|安全備份並停止扣費
Extend by 2 hours|+2 小时续航|延長 2 小時
Google instance release verified · no further charges|已确认 Google 实例释放 · 0 扣费|已確認 Google 執行個體釋放 · 0 扣費
Runtime logs|运行日志|執行日誌
No logs yet|暂无日志|尚無日誌
provisioning|分配中|分配中
bootstrapping|启动中|啟動中
ready|就绪|就緒
busy|处理中|處理中
stopping|停止中|停止中
stopped|已停止|已停止
orphan|未归属|未歸屬
error|错误|錯誤
pending|待处理|待處理
running|运行中|執行中
completed|已完成|已完成
failed|失败|失敗
skipped|已跳过|已略過
queued|排队中|排隊中
downloading|下载中|下載中
connected|已连接|已連線
connecting|连接中|連線中
reconnecting|重新连接中|重新連線中
Saved|已保存|已儲存
Copied|已复制|已複製
Copy|复制|複製
Remove this item?|确认移除此项目？|確定移除此項目？
Loading workspace…|正在加载工作空间…|正在載入工作空間…
Live mode requires the desktop app|请在桌面应用中切换到实盘 Colab。|請在桌面應用程式中切換至正式 Colab。
Desktop windows require the desktop app|请在桌面应用中打开原生 ComfyUI 窗口。|請在桌面應用程式中開啟原生 ComfyUI 視窗。
Model library & pre-downloader|模型库与缺模型预下载|模型庫與缺模型預先下載
Inspect workflow dependencies and download models to Google Drive.|检测工作流依赖，将模型下载到 Google Drive。|檢查工作流相依項目，將模型下載至 Google Drive。
Workflow scanner|工作流依赖检测|工作流相依項目檢查
Import JSON|导入 JSON|匯入 JSON
Load and scan example|加载示例工作流检测|載入並檢測範例工作流
Scan workflow|检测工作流|檢測工作流
Paste workflow JSON|粘贴工作流 JSON|貼上工作流 JSON
Nodes|节点|節點
Missing|缺失待下载|缺少待下載
On Drive|已存在于云盘|已存在於雲端硬碟
Queue all missing models|将缺失模型全部加入下载队列|將缺失模型全部加入下載佇列
Filename|文件名|檔案名稱
Category|类别|類別
Source URL|下载源地址|下載來源網址
Supply a source URL for each missing model.|请为每个缺失模型填写下载地址。|請為每個缺失模型填入下載網址。
Custom download|添加自定义模型|新增自訂模型
Drive destination|目标云盘目录|目標雲端硬碟目錄
Add to queue|加入下载队列|加入下載佇列
Download queue|下载队列|下載佇列
Start CPU downloads to 30 TB Drive|启动 CPU 省积分模式一键下载到 30TB 云盘|啟動 CPU 省點數模式下載至 30TB 雲端硬碟
Download on current instance|在当前已开机实例中下载|在目前執行個體中下載
Queue models to start a download.|加入模型后即可开始下载。|將模型加入佇列後即可開始下載。
Drive inventory|云盘模型清单|雲端硬碟模型清單
Search models|搜索模型|搜尋模型
No matching models|没有匹配的模型|沒有相符的模型
Size|大小|大小
Serverless Workflow API|API 自动唤醒与工作流调用|API 自動喚醒與工作流呼叫
Calls to the local gateway can wake a GPU, mount Drive, run the workflow and return its outputs. The profile controls idle shutdown.|本地网关收到调用后可自动启动 GPU、挂载云盘、运行工作流并返回结果。闲置后按配置自动关机。|本機閘道收到呼叫後可自動啟動 GPU、掛載硬碟、執行工作流並傳回結果。閒置後依設定自動關機。
Auto-wake on API call|API 调用自动唤醒|API 呼叫自動喚醒
Workflow templates|工作流模板|工作流範本
Import API workflow|导入 API 工作流|匯入 API 工作流
Parameter bindings|参数绑定|參數繫結
Parameter|参数|參數
Node|节点|節點
Input|输入|輸入
No parameter bindings|未绑定参数|未繫結參數
Workflow runner|交互式工作流测试|互動式工作流測試
Prompt|提示词|提示詞
Seed|随机种子|隨機種子
Frames|帧数|影格數
Continuity|分镜模式|分鏡模式
First shot|首镜头|首鏡頭
Continue sequence|续接分镜|接續分鏡
Parent task ID|上一个任务 ID|上一個任務 ID
Sequence name|分镜序列名称|分鏡序列名稱
Run test with auto-wake|立即调用测试（未开机将自动拉起）|立即呼叫測試（未開機將自動啟動）
Run result|调用结果|呼叫結果
Request examples|请求示例|請求範例
Import an API workflow to run a test.|请先导入 API 工作流模板。|請先匯入 API 工作流範本。
Name|名称|名稱
Bound profile|绑定配置档|繫結設定檔
Output type|输出格式|輸出格式
Allow continuous shots|支持分镜续接|支援分鏡接續
Accounts, sessions & ports|多账号、会话与端口转发|多帳號、工作階段與連接埠轉送
Manage Google accounts, monitor spending, and forward local ports.|管理 Google 账号、监控扣费，并转发本地端口。|管理 Google 帳號、監控扣費，並轉送本機連接埠。
Google accounts|Google 账号|Google 帳號
Add Google account|添加新 Google 账号|新增 Google 帳號
Adopt CLI account|导入 CLI 账号|匯入 CLI 帳號
Open authorization page|打开授权页面|開啟授權頁面
Authorization code|授权码|授權碼
Complete sign-in|完成登录|完成登入
Sandbox authorization: enter a demo code.|演示沙盒可填写任意测试授权码。|示範沙盒可填入任意測試授權碼。
Future launches use the selected account. Existing instances retain their owner.|新的实例将使用选定账号。已有实例仍归原账号管理。|新執行個體將使用所選帳號。現有執行個體仍由原帳號管理。
Switch account|切换账号|切換帳號
Sessions|会话监控|工作階段監控
No active sessions|暂无运行实例|目前沒有執行中的執行個體
VM watchdog|云端看门狗|雲端監控程式
The VM watchdog can shut down the instance on schedule even after the desktop app closes.|即使关闭本桌面软件，已启用的云端看门狗依然按时关机防扣费。|即使關閉桌面軟體，已啟用的雲端監控程式仍會按時關機防止扣費。
Port forwarding|端口转发|連接埠轉送
New tunnel|新建隧道|新增通道
Session|会话|工作階段
Local port|本地端口|本機連接埠
Remote port|远端端口|遠端連接埠
Health path|健康检查路径|健康檢查路徑
Stop|停止|停止
No tunnels yet|暂无端口隧道|尚無連接埠通道
GGUF deployment & plugins|一键部署大模型与插件|一鍵部署大型模型與外掛
Inspect GGUF files, deploy llama.cpp, and test an OpenAI-compatible endpoint.|检查 GGUF 量化文件，部署 llama.cpp 并测试兼容 OpenAI 的服务。|檢查 GGUF 量化檔案，部署 llama.cpp 並測試相容 OpenAI 的服務。
Model repository|模型仓库|模型儲存庫
Target GPU|目标 GPU|目標 GPU
Inspect and select quantization|检测仓库并自动挑最佳量化|檢測儲存庫並自動挑選最佳量化
Quantization|量化版本|量化版本
Estimated VRAM|预计显存|預估顯示記憶體
Fits|可运行|可執行
Insufficient VRAM|显存不足|顯示記憶體不足
Recommended|推荐|推薦
Deploy llama.cpp|一键部署 llama.cpp|一鍵部署 llama.cpp
Deployments|已部署服务|已部署服務
Chat|对话测试|對話測試
Send|发送|傳送
Message|消息|訊息
You|你|你
Installed plugins|已安装插件|已安裝外掛
Environment & setup|环境引导与设置|環境引導與設定
Verify local dependencies, configure storage, and manage credentials.|检查本机依赖、设置存储路径与管理凭据。|檢查本機相依項目、設定儲存路徑與管理憑證。
Setup checklist|环境检查清单|環境檢查清單
Check environment|检测环境|檢查環境
Install Colab CLI|一键安装 Colab CLI|一鍵安裝 Colab CLI
SSH key|SSH 密钥|SSH 金鑰
Google authorization|Google 账号授权|Google 帳號授權
Available|已就绪|就緒
Action needed|待配置|待設定
Preferences|偏好设置|偏好設定
Minimize to tray|最小化到系统托盘|最小化至系統匣
Sync DramaSmith automatically|自动同步 DramaSmith|自動同步 DramaSmith
Secret vault|凭据保险库|憑證保險庫
rclone unattended config|rclone 无人值守挂载配置|rclone 無人值守掛載設定
Configured|已配置|已設定
Not configured|未配置|未設定
Sandbox stores configuration flags only.|沙盒仅记录凭据配置状态。|沙盒僅記錄憑證設定狀態。
Current session|当前会话|目前工作階段
Default for new profiles|新建配置档的默认策略|新增設定檔的預設策略
Ready|就绪|就緒
Switch to live Colab? Launching an instance may consume compute units.|切换至实盘 Colab？启动实例可能消耗算力积分。|切換至正式 Colab？啟動執行個體可能消耗運算點數。
Account unavailable|账号不可用|帳號無法使用
Select an account and profile first|请先选择账号与配置档|請先選擇帳號與設定檔
Session unavailable|会话不可用|工作階段無法使用
Stop active sessions first|请先停止运行中的会话|請先停止執行中的工作階段
Local port already in use|本地端口已被占用|本機連接埠已被占用
Invalid workflow JSON|工作流 JSON 格式无效|工作流 JSON 格式無效
Automatic wake is disabled|自动唤醒已关闭|自動喚醒已關閉
Select a workflow|请选择工作流|請選擇工作流
Progress|进度|進度
Healthy|正常|正常
Unhealthy|异常|異常
Unknown|未知|未知
Authorization|账号授权|帳號授權
Working…|处理中…|處理中…
Release existing instances before switching backend modes.|请先释放已有实例，再切换后端模式，以便正确核验停止扣费状态。|請先釋放既有執行個體，再切換後端模式，以便正確核驗停止扣費狀態。
Invalid parameter binding|参数绑定无效，请检查节点和字段|參數繫結無效，請檢查節點與欄位
Run test|立即调用测试|立即呼叫測試
Stale usage|用量待刷新|用量待更新
No plugins installed|尚未安装插件|尚未安裝外掛
Drive folder (relative to root)|目标云盘目录（相对根目录）|目標硬碟目錄（相對根目錄）
DramaSmith URL|DramaSmith 地址|DramaSmith 位址
Hugging Face Token|Hugging Face 令牌|Hugging Face 權杖
Civitai API Key|Civitai API 密钥|Civitai API 金鑰
Enter authorization code|请输入授权码|請輸入授權碼
Select a ready session|请选择已就绪的会话|請選擇就緒的工作階段
DramaSmith H3 · BF16 Continuity (G4 96GB)|DramaSmith 文武 H3 · BF16 连续性出片 (G4 96GB)|DramaSmith 文武 H3 · BF16 連續性輸出 (G4 96GB)
Flux / SDXL Image Studio (L4 24GB)|Flux / SDXL 高清生图与精修工作流 (L4 24GB)|Flux / SDXL 高畫質生圖與精修工作流 (L4 24GB)
Wan2.1 / Hunyuan Image-to-Video (A100 80GB)|Wan2.1 / Hunyuan 通用图生视频 (A100 80GB)|Wan2.1 / Hunyuan 通用圖生影片 (A100 80GB)
CPU · Preload models and environment|CPU 零显卡积分 · 提前下载模型与配环境|CPU 零顯示卡點數 · 預先下載模型與設定環境
Restore the H3 environment from Drive and start or continue a shot.|恢复云盘上的文武单采环境，载入首帧或沿用上一段的连续性缓存。|還原硬碟上的文武環境，載入首幀或沿用上一段的連續性快取。
Generate, refine, and upscale images using your saved Drive workflows.|复用 Drive 模型库及已保存的工作流，生成、精修和放大图片。|使用 Drive 模型庫與已儲存的工作流，產生、精修及放大圖片。
Run video workflows on a high-memory A100 and save outputs to Drive.|申请高内存 A100，用本地 SSD 运行视频工作流并将输出保存至云盘。|申請高記憶體 A100，使用本機 SSD 執行影片工作流並將結果儲存至雲端硬碟。
Download missing models to Drive on CPU and update the inventory.|使用 CPU 将所缺模型直接下载到 Drive，并更新模型清单。|使用 CPU 將缺少的模型直接下載至 Drive，並更新模型清單。
Wenwu H3 · BF16 Continuity|文武 H3 · BF16 连续性|文武 H3 · BF16 連續性
SDXL text-to-image|通用 SDXL 文生图|通用 SDXL 文字生圖
First frame|首帧图片|首幀圖片
Negative prompt|负面提示词|負面提示詞
Checkpoint|主模型|主模型
Mock ComfyUI runtime|ComfyUI 沙盒实例|ComfyUI 沙盒執行個體
Sandbox workflow runs simulate inference and output paths.|沙盒可模拟工作流推理与输出路径。|沙盒可模擬工作流推論與輸出路徑。
Open the live endpoint|打开本机服务地址|開啟本機服務位址
Workflow graph|工作流节点图|工作流節點圖
Allow pop-ups to open the preview window|请允许弹出窗口以打开沙盒预览|請允許彈出視窗以開啟沙盒預覽
Sandbox runtime created.|已创建沙盒实例。|已建立沙盒執行個體。
Sandbox instance released.|已释放沙盒实例。|已釋放沙盒執行個體。
Sandbox snapshot saved.|已保存沙盒快照。|已儲存沙盒快照。
Disabled by idle guard|闲置保护已停止实例|閒置防護已停止執行個體
CLI usage update failed|无法刷新 CLI 用量|無法更新 CLI 用量
`;
const entries = new Map<string, string[]>();
for (const row of catalog.trim().split('\n')) {
  const labels = row.split('|');
  for (const label of labels) entries.set(label, labels);
}
export function translate(locale: LocaleCode, key: string) {
  const labels = entries.get(key);
  return labels?.[locale === 'en' ? 0 : locale === 'zh-CN' ? 1 : 2] ?? key;
}
