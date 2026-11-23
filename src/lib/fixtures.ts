import type {
  AppSettings,
  DashboardSnapshot,
  GoogleAccount,
  WorkflowProfile,
  AcceleratorKind,
  SavedApiWorkflow,
  ColabSession,
} from './types';

export const driveRoot = '/content/drive/MyDrive/DramaSmith-H3';
export const defaultGuard = {
  autoStopMinutes: 240,
  idleStopMinutes: 30,
  spendingCapCcu: 150,
  balanceFloorCcu: 20,
  safeBackupBeforeStop: true,
};
export const accelerators: AcceleratorKind[] = ['CPU', 'T4', 'L4', 'A100', 'G4', 'H100', 'V5E1', 'V6E1'];
export const stageLabels = [
  'Allocate GPU and verify VRAM',
  'Mount Google Drive',
  'Restore nodes and models',
  'Connect local tunnel',
  'Check nodes and queue',
];
export function newProfile(
  name: string,
  accelerator: AcceleratorKind = 'G4',
  minVramGb = 96,
): WorkflowProfile {
  return {
    id: crypto.randomUUID(),
    name,
    description: '',
    icon: 'workflow',
    accelerator,
    highMem: accelerator !== 'CPU',
    minVramGb,
    maxRetries: 3,
    driveMountMode: 'colabNative',
    driveRootPath: driveRoot,
    startupMode: accelerator === 'CPU' ? 'cpuPreDownload' : 'managedScript',
    managedScriptRelPath: null,
    comfyPort: 8188,
    managementPort: 8189,
    requiredNodes: [],
    syncDramaSmith: true,
    costGuard: { ...defaultGuard },
    defaultWorkflowJsonPath: null,
  };
}
export function demoAccount(id: string, email: string): GoogleAccount {
  return {
    id,
    email,
    displayName: email.split('@')[0],
    isActive: true,
    planTier: 'CO PRO+',
    balanceCcu: 428.5,
    burnRateHourly: 0,
    hoursRemaining: null,
    activeAssignmentsCount: 0,
    lastUpdatedAt: new Date().toISOString(),
    usageStale: false,
  };
}
export function makeSession(profile: WorkflowProfile, account: GoogleAccount): ColabSession {
  const id = crypto.randomUUID();
  return {
    id,
    name: 'nx-' + profile.accelerator.toLowerCase() + '-' + id.slice(0, 6),
    accountId: account.id,
    accountEmail: account.email,
    endpoint: 'sandbox',
    accelerator: profile.accelerator,
    gpuName: profile.accelerator,
    vramTotalMib: profile.minVramGb * 1024,
    vramUsedMib: 0,
    highMem: profile.highMem,
    profileId: profile.id,
    profileName: profile.name,
    state: 'provisioning',
    stages: stageLabels.map((label, i) => ({
      id: String(i + 1),
      label,
      status: i === 0 ? 'running' : 'pending',
      detail: null,
    })),
    driveMounted: false,
    driveRootPath: profile.driveRootPath,
    driveAuthUrl: null,
    comfyLocalUrl: 'http://127.0.0.1:' + profile.comfyPort,
    managementLocalUrl: profile.managementPort ? 'http://127.0.0.1:' + profile.managementPort : null,
    cloudflareUrl: null,
    comfyReady: false,
    loadedRequiredNodes: [],
    missingRequiredNodes: [],
    queueRunning: 0,
    queuePending: 0,
    watchdogActive: true,
    autoStopAt: profile.costGuard.autoStopMinutes
      ? new Date(Date.now() + profile.costGuard.autoStopMinutes * 60000).toISOString()
      : null,
    idleMinutesLeft: profile.costGuard.idleStopMinutes,
    spentCcu: 0,
    startedAt: new Date().toISOString(),
    verifiedUnassigned: false,
    lastError: null,
    logs: ['Sandbox runtime created.'],
  };
}
export function initialSnapshot(): DashboardSnapshot {
  const settings: AppSettings = {
    locale: 'zh-CN',
    theme: 'dark',
    backendMode: 'real',
    gatewayPort: 18188,
    autoWakeOnApiCall: true,
    dramaSmithApiUrl: 'http://127.0.0.1:8018',
    autoSyncDramaSmith: true,
    defaultDriveRoot: driveRoot,
    defaultCostGuard: { ...defaultGuard },
    minimizeToTray: true,
    hasHfToken: false,
    hasCivitaiToken: false,
    hasRcloneConfig: false,
  };
  const profiles = [
    newProfile('DramaSmith H3 · BF16'),
    newProfile('Flux / SDXL', 'L4', 24),
    newProfile('Wan / Hunyuan', 'A100', 80),
    newProfile('CPU Pre-download', 'CPU', 0),
  ];
  profiles.forEach((p, i) => {
    p.id = ['dramasmith-h3', 'flux-sdxl', 'wan-video', 'cpu-download'][i];
    p.description = [
      'Cinematic video generation',
      'Image generation',
      'Video generation',
      'Zero-GPU model downloads',
    ][i];
  });
  profiles[0].requiredNodes = ['H3C', 'ComfyUI-VideoHelperSuite'];
  const account = demoAccount('studio', 'studio@notebookx.dev');
  const workflow: SavedApiWorkflow = {
    id: 'h3-shot',
    name: 'DramaSmith H3',
    description: 'Cinematic video generation',
    profileId: profiles[0].id,
    outputKind: 'video',
    supportsContinuity: true,
    bindings: [
      {
        key: 'prompt',
        label: 'Prompt',
        nodeId: '6',
        inputField: 'text',
        valueType: 'string',
        defaultValue: '',
      },
      {
        key: 'seed',
        label: 'Seed',
        nodeId: '3',
        inputField: 'seed',
        valueType: 'number',
        defaultValue: 42,
      },
      {
        key: 'frames',
        label: 'Frames',
        nodeId: '7',
        inputField: 'length',
        valueType: 'number',
        defaultValue: 49,
      },
    ],
    apiGraphJson: {
      '6': { class_type: 'CLIPTextEncode', inputs: { text: '' } },
      '3': { class_type: 'KSampler', inputs: { seed: 42 } },
      '7': { class_type: 'EmptyLatentVideo', inputs: { length: 49 } },
    },
    updatedAt: new Date().toISOString(),
  };
  return {
    settings,
    env: {
      backendMode: 'mock',
      pythonFound: true,
      pythonPath: 'Python 3.11',
      uvFound: true,
      uvPath: 'uv',
      colabCliFound: true,
      colabCliPath: 'google-colab-cli',
      colabCliVersion: 'sandbox',
      sshFound: true,
      sshPath: 'OpenSSH',
      sshKeyReady: true,
      sshKeyPath: '~/.ssh/id_ed25519',
      gatewayRunning: true,
      gatewayUrl: 'http://127.0.0.1:18188',
      dramaSmithReachable: true,
      dramaSmithUrl: settings.dramaSmithApiUrl,
    },
    accounts: [],
    profiles,
    sessions: [],
    tunnels: [],
    downloads: [],
    llamaDeployments: [],
    apiWorkflows: [workflow],
    driveModels: [],
    plugins: [
      {
        id: 'llama.cpp',
        version: '1.0',
        name: { en: 'llama.cpp' },
        description: {
          en: 'OpenAI-compatible GGUF inference',
          'zh-CN': '兼容 OpenAI 的 GGUF 推理服务',
          'zh-TW': '相容 OpenAI 的 GGUF 推論服務',
        },
        defaultAccelerator: 'L4',
        minVramMib: 15360,
        highMem: true,
        scriptPath: 'plugins/llama.cpp',
        builtin: true,
        endpoints: [
          { id: 'api', label: 'OpenAI', port: 8000, path: '/v1', healthPath: '/health', kind: 'api' },
        ],
      },
    ],
  };
}
