#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendMode {
    Real,
    Mock,
}

impl Default for BackendMode {
    fn default() -> Self {
        Self::Real
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocaleCode {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "zh-TW")]
    ZhTw,
    #[serde(rename = "en")]
    En,
}

impl Default for LocaleCode {
    fn default() -> Self {
        Self::ZhCn
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AcceleratorKind {
    #[serde(rename = "CPU")]
    Cpu,
    #[serde(rename = "T4")]
    T4,
    #[serde(rename = "L4")]
    L4,
    #[serde(rename = "G4")]
    G4,
    #[serde(rename = "A100")]
    A100,
    #[serde(rename = "H100")]
    H100,
    #[serde(rename = "V5E1")]
    V5e1,
    #[serde(rename = "V6E1")]
    V6e1,
}

impl AcceleratorKind {
    pub fn cli_flag_pair(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Cpu => None,
            Self::T4 => Some(("--gpu", "T4")),
            Self::L4 => Some(("--gpu", "L4")),
            Self::G4 => Some(("--gpu", "G4")),
            Self::A100 => Some(("--gpu", "A100")),
            Self::H100 => Some(("--gpu", "H100")),
            Self::V5e1 => Some(("--tpu", "v5e1")),
            Self::V6e1 => Some(("--tpu", "v6e1")),
        }
    }

    pub fn nominal_vram_mib(self) -> u32 {
        match self {
            Self::Cpu | Self::V5e1 | Self::V6e1 => 0,
            Self::T4 => 15_360,
            Self::L4 => 23_034,
            Self::G4 => 97_871,
            Self::A100 => 40_960,
            Self::H100 => 81_920,
        }
    }

    pub fn is_gpu(self) -> bool {
        matches!(self, Self::T4 | Self::L4 | Self::G4 | Self::A100 | Self::H100)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStatus {
    pub backendMode: BackendMode,
    pub pythonFound: bool,
    pub pythonPath: Option<String>,
    pub uvFound: bool,
    pub uvPath: Option<String>,
    pub colabCliFound: bool,
    pub colabCliPath: Option<String>,
    pub colabCliVersion: Option<String>,
    pub sshFound: bool,
    pub sshPath: Option<String>,
    pub sshKeyReady: bool,
    pub sshKeyPath: Option<String>,
    pub gatewayRunning: bool,
    pub gatewayUrl: String,
    pub dramaSmithReachable: bool,
    pub dramaSmithUrl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAccount {
    pub id: String,
    pub email: String,
    pub displayName: Option<String>,
    pub isActive: bool,
    pub planTier: String,
    pub balanceCcu: Option<f64>,
    pub burnRateHourly: Option<f64>,
    pub hoursRemaining: Option<f64>,
    pub activeAssignmentsCount: u32,
    pub lastUpdatedAt: Option<String>,
    pub usageStale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostGuardPolicy {
    pub autoStopMinutes: Option<u32>,
    pub idleStopMinutes: Option<u32>,
    pub spendingCapCcu: Option<f64>,
    pub balanceFloorCcu: Option<f64>,
    pub safeBackupBeforeStop: bool,
}

impl Default for CostGuardPolicy {
    fn default() -> Self {
        Self {
            autoStopMinutes: Some(240),
            idleStopMinutes: Some(30),
            spendingCapCcu: Some(150.0),
            balanceFloorCcu: Some(20.0),
            safeBackupBeforeStop: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DriveMountMode {
    ColabNative,
    RcloneUnattended,
    Disabled,
}

impl Default for DriveMountMode {
    fn default() -> Self {
        Self::ColabNative
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileStartupMode {
    ManagedScript,
    ComfySnapshot,
    CpuPreDownload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub accelerator: AcceleratorKind,
    pub highMem: bool,
    pub minVramGb: u32,
    pub maxRetries: u32,
    pub driveMountMode: DriveMountMode,
    pub driveRootPath: String,
    pub startupMode: ProfileStartupMode,
    pub managedScriptRelPath: Option<String>,
    pub comfyPort: u16,
    pub managementPort: Option<u16>,
    pub requiredNodes: Vec<String>,
    pub syncDramaSmith: bool,
    pub costGuard: CostGuardPolicy,
    pub defaultWorkflowJsonPath: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupStage {
    pub id: String,
    pub label: String,
    pub status: StageStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionLifecycle {
    Provisioning,
    Bootstrapping,
    Ready,
    Busy,
    Stopping,
    Stopped,
    Orphan,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColabSession {
    pub id: String,
    pub name: String,
    pub accountId: String,
    pub accountEmail: String,
    pub endpoint: String,
    pub accelerator: AcceleratorKind,
    pub gpuName: Option<String>,
    pub vramTotalMib: Option<u32>,
    pub vramUsedMib: Option<u32>,
    pub highMem: bool,
    pub profileId: Option<String>,
    pub profileName: Option<String>,
    pub state: SessionLifecycle,
    pub stages: Vec<StartupStage>,
    pub driveMounted: bool,
    pub driveRootPath: Option<String>,
    pub driveAuthUrl: Option<String>,
    pub comfyLocalUrl: Option<String>,
    pub managementLocalUrl: Option<String>,
    pub cloudflareUrl: Option<String>,
    pub comfyReady: bool,
    pub loadedRequiredNodes: Vec<String>,
    pub missingRequiredNodes: Vec<String>,
    pub queueRunning: u32,
    pub queuePending: u32,
    pub watchdogActive: bool,
    pub autoStopAt: Option<String>,
    pub idleMinutesLeft: Option<u32>,
    pub spentCcu: f64,
    pub startedAt: String,
    pub verifiedUnassigned: bool,
    pub lastError: Option<String>,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TunnelStatus {
    Connecting,
    Connected,
    Reconnecting,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortTunnel {
    pub id: String,
    pub sessionName: String,
    pub remotePort: u16,
    pub localPort: u16,
    pub label: String,
    pub status: TunnelStatus,
    pub healthy: Option<bool>,
    pub healthPath: Option<String>,
    pub reconnectCount: u32,
    pub lastError: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCategory {
    Checkpoints,
    DiffusionModels,
    Loras,
    Vae,
    TextEncoders,
    Clip,
    ClipVision,
    Controlnet,
    UpscaleModels,
    SemanticBridge,
    CustomNodes,
    Other,
}

impl ModelCategory {
    pub fn drive_subdir(self) -> &'static str {
        match self {
            Self::Checkpoints => "models/checkpoints",
            Self::DiffusionModels => "models/diffusion_models",
            Self::Loras => "models/loras",
            Self::Vae => "models/vae",
            Self::TextEncoders => "models/text_encoders",
            Self::Clip => "models/clip",
            Self::ClipVision => "models/clip_vision",
            Self::Controlnet => "models/controlnet",
            Self::UpscaleModels => "models/upscale_models",
            Self::SemanticBridge => "models/semantic_bridge",
            Self::CustomNodes => "plugins",
            Self::Other => "models/other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveModelItem {
    pub filename: String,
    pub relativePath: String,
    pub category: ModelCategory,
    pub sizeBytes: u64,
    pub updatedAt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowModelRequirement {
    pub filename: String,
    pub category: ModelCategory,
    pub nodeType: String,
    pub presentOnDrive: bool,
    pub matchedDrivePath: Option<String>,
    pub suggestedUrl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowScanResult {
    pub workflowName: String,
    pub nodeCount: usize,
    pub requiredCustomNodes: Vec<String>,
    pub models: Vec<WorkflowModelRequirement>,
    pub presentCount: usize,
    pub missingCount: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadTask {
    pub id: String,
    pub filename: String,
    pub sourceUrl: String,
    pub category: ModelCategory,
    pub targetDriveDir: String,
    pub status: DownloadStatus,
    pub progressPercent: u8,
    pub downloadedBytes: u64,
    pub totalBytes: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowParamBinding {
    pub key: String,
    pub label: String,
    pub nodeId: String,
    pub inputField: String,
    pub valueType: String,
    pub defaultValue: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedApiWorkflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub profileId: String,
    pub outputKind: String,
    pub supportsContinuity: bool,
    pub bindings: Vec<WorkflowParamBinding>,
    pub apiGraphJson: serde_json::Value,
    pub updatedAt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunRequest {
    pub workflowId: Option<String>,
    pub profileId: Option<String>,
    pub rawPromptGraph: Option<serde_json::Value>,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    pub continuityMode: Option<String>,
    pub parentTaskId: Option<String>,
    pub sequenceName: Option<String>,
    #[serde(default = "default_true")]
    pub autoWake: bool,
    #[serde(default = "default_true")]
    pub waitForCompletion: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunResult {
    pub jobId: String,
    pub promptId: Option<String>,
    pub sessionName: String,
    pub coldStarted: bool,
    pub status: String,
    pub outputs: Vec<String>,
    pub driveSequencePath: Option<String>,
    pub elapsedSeconds: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GgufVariantEstimate {
    pub quant: String,
    pub files: Vec<String>,
    pub sizeBytes: u64,
    pub estimatedVramMib: u32,
    pub recommendedCtx: u32,
    pub kvCacheType: String,
    pub fitsCurrentTarget: bool,
    pub isAutoSelected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GgufRepoInspection {
    pub repoId: String,
    pub targetVramMib: u32,
    pub variants: Vec<GgufVariantEstimate>,
    pub mmprojFiles: Vec<String>,
    pub autoPickedQuant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDescriptor {
    pub id: String,
    pub version: String,
    pub name: HashMap<String, String>,
    pub description: HashMap<String, String>,
    pub defaultAccelerator: AcceleratorKind,
    pub minVramMib: u32,
    pub highMem: bool,
    pub scriptPath: String,
    pub builtin: bool,
    pub endpoints: Vec<PluginEndpointSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEndpointSpec {
    pub id: String,
    pub label: String,
    pub port: u16,
    pub path: String,
    pub healthPath: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlamaDeployment {
    pub id: String,
    pub sessionName: String,
    pub repoId: String,
    pub chosenQuant: String,
    pub ctxSize: u32,
    pub localPort: u16,
    pub apiBaseUrl: String,
    pub webChatUrl: String,
    pub status: String,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub locale: LocaleCode,
    pub theme: ThemeMode,
    pub backendMode: BackendMode,
    pub gatewayPort: u16,
    pub autoWakeOnApiCall: bool,
    pub dramaSmithApiUrl: String,
    pub autoSyncDramaSmith: bool,
    pub defaultDriveRoot: String,
    pub defaultCostGuard: CostGuardPolicy,
    pub minimizeToTray: bool,
    pub hasHfToken: bool,
    pub hasCivitaiToken: bool,
    pub hasRcloneConfig: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: LocaleCode::ZhCn,
            theme: ThemeMode::Dark,
            backendMode: BackendMode::Real,
            gatewayPort: 18188,
            autoWakeOnApiCall: true,
            dramaSmithApiUrl: "http://127.0.0.1:8018".to_string(),
            autoSyncDramaSmith: true,
            defaultDriveRoot: "/content/drive/MyDrive/DramaSmith-H3".to_string(),
            defaultCostGuard: CostGuardPolicy::default(),
            minimizeToTray: true,
            hasHfToken: false,
            hasCivitaiToken: false,
            hasRcloneConfig: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub env: EnvironmentStatus,
    pub settings: AppSettings,
    pub accounts: Vec<GoogleAccount>,
    pub profiles: Vec<WorkflowProfile>,
    pub sessions: Vec<ColabSession>,
    pub tunnels: Vec<PortTunnel>,
    pub driveModels: Vec<DriveModelItem>,
    pub downloads: Vec<ModelDownloadTask>,
    pub apiWorkflows: Vec<SavedApiWorkflow>,
    pub plugins: Vec<PluginDescriptor>,
    pub llamaDeployments: Vec<LlamaDeployment>,
}
