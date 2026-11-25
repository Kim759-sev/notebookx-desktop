export type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export type BackendMode = 'real' | 'mock';

export type LocaleCode = 'zh-CN' | 'zh-TW' | 'en';

export type ThemeMode = 'light' | 'dark' | 'system';

export type AcceleratorKind = 'CPU' | 'T4' | 'L4' | 'G4' | 'A100' | 'H100' | 'V5E1' | 'V6E1';

export interface EnvironmentStatus {
  backendMode: BackendMode;
  pythonFound: boolean;
  pythonPath: string | null;
  uvFound: boolean;
  uvPath: string | null;
  colabCliFound: boolean;
  colabCliPath: string | null;
  colabCliVersion: string | null;
  sshFound: boolean;
  sshPath: string | null;
  sshKeyReady: boolean;
  sshKeyPath: string | null;
  gatewayRunning: boolean;
  gatewayUrl: string;
  dramaSmithReachable: boolean;
  dramaSmithUrl: string;
}

export interface GoogleAccount {
  id: string;
  email: string;
  displayName: string | null;
  isActive: boolean;
  planTier: string;
  balanceCcu: number | null;
  burnRateHourly: number | null;
  hoursRemaining: number | null;
  activeAssignmentsCount: number;
  lastUpdatedAt: string | null;
  usageStale: boolean;
}

export interface CostGuardPolicy {
  autoStopMinutes: number | null;
  idleStopMinutes: number | null;
  spendingCapCcu: number | null;
  balanceFloorCcu: number | null;
  safeBackupBeforeStop: boolean;
}

export type DriveMountMode = 'colabNative' | 'rcloneUnattended' | 'disabled';

export type ProfileStartupMode = 'managedScript' | 'comfySnapshot' | 'cpuPreDownload';

export interface WorkflowProfile {
  id: string;
  name: string;
  description: string;
  icon: string;
  accelerator: AcceleratorKind;
  highMem: boolean;
  minVramGb: number;
  maxRetries: number;
  driveMountMode: DriveMountMode;
  driveRootPath: string;
  startupMode: ProfileStartupMode;
  managedScriptRelPath: string | null;
  comfyPort: number;
  managementPort: number | null;
  requiredNodes: Array<string>;
  syncDramaSmith: boolean;
  costGuard: CostGuardPolicy;
  defaultWorkflowJsonPath: string | null;
}

export type StageStatus = 'pending' | 'running' | 'completed' | 'warning' | 'failed' | 'skipped';

export interface StartupStage {
  id: string;
  label: string;
  status: StageStatus;
  detail: string | null;
}

export type SessionLifecycle =
  | 'provisioning'
  | 'bootstrapping'
  | 'ready'
  | 'busy'
  | 'stopping'
  | 'stopped'
  | 'orphan'
  | 'error';

export interface ColabSession {
  id: string;
  name: string;
  accountId: string;
  accountEmail: string;
  endpoint: string;
  accelerator: AcceleratorKind;
  gpuName: string | null;
  vramTotalMib: number | null;
  vramUsedMib: number | null;
  highMem: boolean;
  profileId: string | null;
  profileName: string | null;
  state: SessionLifecycle;
  stages: Array<StartupStage>;
  driveMounted: boolean;
  driveRootPath: string | null;
  driveAuthUrl: string | null;
  comfyLocalUrl: string | null;
  managementLocalUrl: string | null;
  cloudflareUrl: string | null;
  comfyReady: boolean;
  loadedRequiredNodes: Array<string>;
  missingRequiredNodes: Array<string>;
  queueRunning: number;
  queuePending: number;
  watchdogActive: boolean;
  autoStopAt: string | null;
  idleMinutesLeft: number | null;
  spentCcu: number;
  startedAt: string;
  verifiedUnassigned: boolean;
  lastError: string | null;
  logs: Array<string>;
}

export type TunnelStatus = 'connecting' | 'connected' | 'reconnecting' | 'stopped' | 'failed';

export interface PortTunnel {
  id: string;
  sessionName: string;
  remotePort: number;
  localPort: number;
  label: string;
  status: TunnelStatus;
  healthy: boolean | null;
  healthPath: string | null;
  reconnectCount: number;
  lastError: string | null;
}

export type ModelCategory =
  | 'checkpoints'
  | 'diffusion_models'
  | 'loras'
  | 'vae'
  | 'text_encoders'
  | 'clip'
  | 'clip_vision'
  | 'controlnet'
  | 'upscale_models'
  | 'semantic_bridge'
  | 'custom_nodes'
  | 'other';

export interface DriveModelItem {
  filename: string;
  relativePath: string;
  category: ModelCategory;
  sizeBytes: number;
  updatedAt: string | null;
}

export interface WorkflowModelRequirement {
  filename: string;
  category: ModelCategory;
  nodeType: string;
  presentOnDrive: boolean;
  matchedDrivePath: string | null;
  suggestedUrl: string | null;
}

export interface WorkflowScanResult {
  workflowName: string;
  nodeCount: number;
  requiredCustomNodes: Array<string>;
  models: Array<WorkflowModelRequirement>;
  presentCount: number;
  missingCount: number;
}

export type DownloadStatus = 'queued' | 'downloading' | 'completed' | 'failed';

export interface ModelDownloadTask {
  id: string;
  filename: string;
  sourceUrl: string;
  category: ModelCategory;
  targetDriveDir: string;
  status: DownloadStatus;
  progressPercent: number;
  downloadedBytes: number;
  totalBytes: number | null;
  error: string | null;
}

export interface WorkflowParamBinding {
  key: string;
  label: string;
  nodeId: string;
  inputField: string;
  valueType: string;
  defaultValue: JsonValue;
}

export interface SavedApiWorkflow {
  id: string;
  name: string;
  description: string;
  profileId: string;
  outputKind: string;
  supportsContinuity: boolean;
  bindings: Array<WorkflowParamBinding>;
  apiGraphJson: JsonValue;
  updatedAt: string;
}

export interface WorkflowRunRequest {
  workflowId: string | null;
  profileId: string | null;
  rawPromptGraph: JsonValue | null;
  params: Record<string, JsonValue>;
  continuityMode: string | null;
  parentTaskId: string | null;
  sequenceName: string | null;
  autoWake: boolean;
  waitForCompletion: boolean;
}

export interface WorkflowRunResult {
  jobId: string;
  promptId: string | null;
  sessionName: string;
  coldStarted: boolean;
  status: string;
  outputs: Array<string>;
  driveSequencePath: string | null;
  elapsedSeconds: number;
  error: string | null;
}

export interface GgufVariantEstimate {
  quant: string;
  files: Array<string>;
  sizeBytes: number;
  estimatedVramMib: number;
  recommendedCtx: number;
  kvCacheType: string;
  fitsCurrentTarget: boolean;
  isAutoSelected: boolean;
}

export interface GgufRepoInspection {
  repoId: string;
  targetVramMib: number;
  variants: Array<GgufVariantEstimate>;
  mmprojFiles: Array<string>;
  autoPickedQuant: string | null;
}

export interface PluginDescriptor {
  id: string;
  version: string;
  name: Record<string, string>;
  description: Record<string, string>;
  defaultAccelerator: AcceleratorKind;
  minVramMib: number;
  highMem: boolean;
  scriptPath: string;
  builtin: boolean;
  endpoints: Array<PluginEndpointSpec>;
}

export interface PluginEndpointSpec {
  id: string;
  label: string;
  port: number;
  path: string;
  healthPath: string | null;
  kind: string;
}

export interface LlamaDeployment {
  id: string;
  sessionName: string;
  repoId: string;
  chosenQuant: string;
  ctxSize: number;
  localPort: number;
  apiBaseUrl: string;
  webChatUrl: string;
  status: string;
  logs: Array<string>;
}

export interface AppSettings {
  locale: LocaleCode;
  theme: ThemeMode;
  backendMode: BackendMode;
  gatewayPort: number;
  autoWakeOnApiCall: boolean;
  dramaSmithApiUrl: string;
  autoSyncDramaSmith: boolean;
  defaultDriveRoot: string;
  defaultCostGuard: CostGuardPolicy;
  minimizeToTray: boolean;
  hasHfToken: boolean;
  hasCivitaiToken: boolean;
  hasRcloneConfig: boolean;
}

export interface DashboardSnapshot {
  env: EnvironmentStatus;
  settings: AppSettings;
  accounts: Array<GoogleAccount>;
  profiles: Array<WorkflowProfile>;
  sessions: Array<ColabSession>;
  tunnels: Array<PortTunnel>;
  driveModels: Array<DriveModelItem>;
  downloads: Array<ModelDownloadTask>;
  apiWorkflows: Array<SavedApiWorkflow>;
  plugins: Array<PluginDescriptor>;
  llamaDeployments: Array<LlamaDeployment>;
}
