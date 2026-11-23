import { demoAccount, initialSnapshot, makeSession } from './fixtures';
import type { Args, Command, Result } from './api';
import type {
  ColabSession,
  DashboardSnapshot,
  GgufRepoInspection,
  ModelCategory,
  ModelDownloadTask,
  PortTunnel,
  WorkflowScanResult,
} from './types';

const storageKey = 'notebookx.preview.v1';
let data: DashboardSnapshot;
try {
  const saved = localStorage.getItem(storageKey);
  data = saved ? JSON.parse(saved) : initialSnapshot();
} catch {
  data = initialSnapshot();
}
function persist() {
  try {
    localStorage.setItem(storageKey, JSON.stringify(data));
  } catch {
    /* Private windows can still use the sandbox. */
  }
}
function upsert<T extends { id: string }>(rows: T[], item: T) {
  const index = rows.findIndex((row) => row.id === item.id);
  if (index < 0) rows.push(item);
  else rows[index] = item;
  return rows;
}
function session(name: string) {
  const found = data.sessions.find((s) => s.name === name);
  if (!found) throw Error('Session unavailable');
  return found;
}
function activate(id: string) {
  if (!data.accounts.some((a) => a.id === id)) throw Error('Account unavailable');
  data.accounts.forEach((a) => (a.isActive = a.id === id));
  return data.accounts;
}
function addAccount() {
  const id = crypto.randomUUID(),
    account = demoAccount(id, 'studio-' + id.slice(0, 4) + '@notebookx.dev');
  data.accounts.push(account);
  activate(id);
  return account;
}
function launch(profileId: string) {
  const p = data.profiles.find((p) => p.id === profileId),
    a = data.accounts.find((a) => a.isActive);
  if (!p || !a) throw Error('Select an account and profile first');
  const existing = data.sessions.find(
    (s) => s.profileId === profileId && s.accountId === a.id && !['stopped', 'error'].includes(s.state),
  );
  if (existing) return existing;
  const s = makeSession(p, a);
  data.sessions.unshift(s);
  return s;
}
function tunnel(input: Args<'create_port_tunnel'>) {
  const s = session(input.sessionName);
  if (s.state === 'stopped') throw Error('Session unavailable');
  if (data.tunnels.some((t) => t.localPort === input.localPort && t.status !== 'stopped'))
    throw Error('Local port already in use');
  const t: PortTunnel = {
    ...input,
    id: crypto.randomUUID(),
    status: 'connected',
    healthy: true,
    reconnectCount: 0,
    lastError: null,
  };
  data.tunnels.unshift(t);
  return t;
}
function ready(s: ColabSession) {
  s.state = 'ready';
  s.driveMounted = true;
  s.comfyReady = data.profiles.find((p) => p.id === s.profileId)?.startupMode !== 'cpuPreDownload';
  s.loadedRequiredNodes = data.profiles.find((p) => p.id === s.profileId)?.requiredNodes ?? [];
  s.stages.forEach((st) => (st.status = 'completed'));
  for (const url of [s.comfyLocalUrl, s.managementLocalUrl]) {
    if (!url || !s.comfyReady) continue;
    const port = Number(new URL(url).port);
    if (!data.tunnels.some((t) => t.localPort === port && t.status !== 'stopped'))
      tunnel({
        sessionName: s.name,
        remotePort: port,
        localPort: port,
        label: url === s.comfyLocalUrl ? 'ComfyUI' : 'Management',
        healthPath: '/',
      });
  }
}
function tick() {
  for (const s of data.sessions) {
    if (['provisioning', 'bootstrapping'].includes(s.state)) {
      const step = Math.floor((Date.now() - Date.parse(s.startedAt)) / 1300);
      s.stages.forEach((st, i) => {
        const status = i < step ? 'completed' : i === step ? 'running' : 'pending';
        if (status === 'completed' && st.status !== status) s.logs.push(st.label);
        st.status = status;
      });
      if (step >= s.stages.length) ready(s);
      else s.state = step ? 'bootstrapping' : 'provisioning';
    }
    if (s.state === 'ready' && s.autoStopAt && Date.parse(s.autoStopAt) <= Date.now()) stop(s.name);
  }
  for (const task of data.downloads.filter((t) => t.status === 'downloading')) {
    const since =
      (task as ModelDownloadTask & { startMs?: number }).startMs ??
      (Number(task.id.split('_')[1]) || Date.now());
    task.progressPercent = Math.min(100, Math.max(1, Math.floor((Date.now() - since) / 150)));
    task.downloadedBytes = Math.floor(((task.totalBytes ?? 0) * task.progressPercent) / 100);
    if (task.progressPercent === 100) {
      task.status = 'completed';
      if (!data.driveModels.some((m) => m.filename === task.filename))
        data.driveModels.push({
          filename: task.filename,
          relativePath: task.targetDriveDir + '/' + task.filename,
          category: task.category,
          sizeBytes: task.totalBytes ?? 0,
          updatedAt: new Date().toISOString(),
        });
    }
  }
  for (const a of data.accounts) {
    const active = data.sessions.filter(
      (s) => s.accountId === a.id && !['stopped', 'error'].includes(s.state),
    );
    a.activeAssignmentsCount = active.length;
    a.burnRateHourly = active.reduce((v, s) => v + (s.accelerator === 'CPU' ? 0 : 8.6), 0);
    a.hoursRemaining = a.burnRateHourly ? (a.balanceCcu ?? 0) / a.burnRateHourly : null;
  }
}
function stop(name: string) {
  const s = session(name);
  s.state = 'stopped';
  s.comfyReady = false;
  s.verifiedUnassigned = true;
  s.watchdogActive = false;
  s.driveMounted = false;
  s.logs.push('Sandbox instance released.');
  data.tunnels
    .filter((t) => t.sessionName === name)
    .forEach((t) => {
      t.status = 'stopped';
      t.healthy = false;
    });
  return s;
}
const knownUrls: Record<string, string> = {
  'ae.safetensors': 'https://huggingface.co/black-forest-labs/FLUX.1-dev/resolve/main/ae.safetensors',
  't5xxl_fp16.safetensors':
    'https://huggingface.co/comfyanonymous/flux_text_encoders/resolve/main/t5xxl_fp16.safetensors',
  'clip_l.safetensors':
    'https://huggingface.co/comfyanonymous/flux_text_encoders/resolve/main/clip_l.safetensors',
};
function scan(workflowName: string, raw: string): WorkflowScanResult {
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw Error('Invalid workflow JSON');
  }
  if (!parsed || typeof parsed !== 'object') throw Error('Invalid workflow JSON');
  const graph = parsed.prompt ?? parsed;
  const nodes: unknown[] = Array.isArray(graph.nodes) ? graph.nodes : Object.values(graph);
  const models: WorkflowScanResult['models'] = [],
    custom = new Set<string>();
  let nodeCount = 0;
  for (const node of nodes) {
    if (!node || typeof node !== 'object') continue;
    const record = node as Record<string, unknown>;
    const type = String(record.class_type ?? record.type ?? '');
    if (!type) continue;
    nodeCount++;
    if (
      !/^(CLIPTextEncode|KSampler|CheckpointLoaderSimple|VAELoader|UNETLoader|DualCLIPLoader|CLIPLoader|LoraLoader|SaveImage|VAEDecode|EmptyLatentImage|ControlNetLoader|LoadImage)$/.test(
        type,
      )
    )
      custom.add(type);
    const visit = (v: unknown, key: string) => {
      if (v && typeof v === 'object') {
        Object.entries(v).forEach(([k, x]) => visit(x, k));
        return;
      }
      if (typeof v !== 'string' || !/\.(safetensors|ckpt|gguf|bin|pt|pth)$/i.test(v)) return;
      const filename = v.split(/[\\/]/).pop()!;
      if (models.some((m) => m.filename === filename)) return;
      const hint = (type + ' ' + key).toLowerCase();
      const category: ModelCategory = hint.includes('lora')
        ? 'loras'
        : hint.includes('vae')
          ? 'vae'
          : hint.includes('clip') || hint.includes('text_encoder')
            ? 'text_encoders'
            : hint.includes('controlnet')
              ? 'controlnet'
              : hint.includes('unet') || hint.includes('diffusion')
                ? 'diffusion_models'
                : 'checkpoints';
      const match = data.driveModels.find((m) => m.filename === filename);
      models.push({
        filename,
        category,
        nodeType: type,
        presentOnDrive: !!match,
        matchedDrivePath: match?.relativePath ?? null,
        suggestedUrl: knownUrls[filename] ?? null,
      });
    };
    visit(record.inputs, '');
    visit(record.widgets_values, '');
  }
  if (!nodeCount) throw Error('Invalid workflow JSON');
  const presentCount = models.filter((m) => m.presentOnDrive).length;
  return {
    workflowName,
    nodeCount,
    models,
    requiredCustomNodes: [...custom],
    presentCount,
    missingCount: models.length - presentCount,
  };
}
function inspect(repoId: string, targetVramMib: number): GgufRepoInspection {
  const variants = [
    ['Q2_K', 12],
    ['Q3_K_M', 16],
    ['Q4_K_M', 20],
    ['Q5_K_M', 24],
    ['Q6_K', 28],
    ['Q8_0', 35],
  ].map(([q, size]) => ({
    quant: String(q),
    files: [repoId.split('/').pop() + '-' + q + '.gguf'],
    sizeBytes: Number(size) * 2 ** 30,
    estimatedVramMib: (Number(size) + 2) * 1024,
    recommendedCtx: 8192,
    kvCacheType: 'q8_0',
    fitsCurrentTarget: (Number(size) + 2) * 1024 <= targetVramMib,
    isAutoSelected: false,
  }));
  const best = variants.filter((v) => v.fitsCurrentTarget).pop();
  if (best) best.isAutoSelected = true;
  return { repoId, targetVramMib, variants, mmprojFiles: [], autoPickedQuant: best?.quant ?? null };
}
const handlers: { [K in Command]: (args: Args<K>) => Result<K> } = {
  get_dashboard_snapshot: () => {
    tick();
    return data;
  },
  check_environment: () => data.env,
  install_colab_cli: () => {
    data.env.colabCliFound = true;
    return data.env;
  },
  update_settings: ({ settings }) => {
    if (settings.backendMode === 'real') throw Error('Live mode requires the desktop app');
    data.settings = settings;
    data.env.gatewayUrl = 'http://127.0.0.1:' + settings.gatewayPort;
    data.env.dramaSmithUrl = settings.dramaSmithApiUrl;
    return settings;
  },
  save_secret: ({ name, value }) => {
    const field = /rclone/i.test(name)
      ? 'hasRcloneConfig'
      : /civitai/i.test(name)
        ? 'hasCivitaiToken'
        : 'hasHfToken';
    data.settings[field] = !!value;
    return data.settings;
  },
  start_account_login: () => 'sandbox:oauth',
  complete_account_login: ({ authCode }) => {
    if (!authCode.trim()) throw Error('Enter authorization code');
    return addAccount();
  },
  adopt_cli_account: addAccount,
  switch_account: ({ accountId }) => activate(accountId),
  remove_account: ({ accountId }) => {
    if (data.sessions.some((s) => s.accountId === accountId && !['stopped', 'error'].includes(s.state)))
      throw Error('Stop active sessions first');
    data.accounts = data.accounts.filter((a) => a.id !== accountId);
    if (data.accounts.length && !data.accounts.some((a) => a.isActive)) activate(data.accounts[0].id);
    return data.accounts;
  },
  refresh_accounts_usage: () => {
    tick();
    data.accounts.forEach((a) => (a.lastUpdatedAt = new Date().toISOString()));
    return data.accounts;
  },
  save_workflow_profile: ({ profile }) => upsert(data.profiles, profile),
  delete_workflow_profile: ({ profileId }) =>
    (data.profiles = data.profiles.filter((p) => p.id !== profileId)),
  launch_profile_session: ({ profileId }) => launch(profileId),
  stop_session: ({ sessionName }) => stop(sessionName),
  extend_session_deadline: ({ sessionName, addMinutes }) => {
    const s = session(sessionName);
    s.autoStopAt = new Date(
      Math.max(Date.now(), Date.parse(s.autoStopAt ?? '') || 0) + addMinutes * 60000,
    ).toISOString();
    return s;
  },
  save_comfy_environment_snapshot: ({ sessionName }) => {
    const s = session(sessionName);
    s.logs.push('Sandbox snapshot saved.');
    return s.driveRootPath + '/snapshots/' + s.name;
  },
  open_comfy_desktop_window: ({ url }) => {
    const s = data.sessions.find((s) => s.comfyLocalUrl === url && s.comfyReady);
    if (!s) throw Error('Session unavailable');
    const target = new URL(window.location.href);
    target.hash = '';
    target.searchParams.set('runtimePreview', s.name);
    const popup = window.open('about:blank', '_blank');
    if (!popup) throw Error('Allow pop-ups to open the preview window');
    popup.opener = null;
    popup.location.replace(target.href);
  },
  sync_dramasmith_endpoint: ({ endpointUrl }) => endpointUrl,
  create_port_tunnel: tunnel,
  stop_port_tunnel: ({ tunnelId }) => {
    const t = data.tunnels.find((t) => t.id === tunnelId);
    if (t) {
      t.status = 'stopped';
      t.healthy = false;
    }
  },
  scan_workflow_models: ({ workflowName, workflowJson }) => scan(workflowName, workflowJson),
  enqueue_model_downloads: ({ items }) => {
    for (const item of items) {
      if (
        data.downloads.some(
          (t) =>
            t.filename === item.filename &&
            t.targetDriveDir === item.targetDriveDir &&
            t.status !== 'failed',
        )
      )
        continue;
      data.downloads.push({
        ...item,
        id: crypto.randomUUID(),
        status: 'queued',
        progressPercent: 0,
        downloadedBytes: 0,
        totalBytes: null,
        error: null,
      });
    }
    return data.downloads;
  },
  start_model_downloads: ({ useCpuMode, sessionName }) => {
    if (!useCpuMode && (!sessionName || !['ready', 'busy'].includes(session(sessionName).state)))
      throw Error('Select a ready session');
    for (const t of data.downloads.filter((t) => ['queued', 'failed'].includes(t.status))) {
      Object.assign(t, { startMs: Date.now() });
      t.status = 'downloading';
      t.totalBytes = 268435456;
    }
    return data.downloads;
  },
  remove_model_download: ({ taskId }) =>
    (data.downloads = data.downloads.filter((t) => t.id !== taskId)),
  save_api_workflow: ({ workflow }) => upsert(data.apiWorkflows, workflow),
  delete_api_workflow: ({ workflowId }) =>
    (data.apiWorkflows = data.apiWorkflows.filter((w) => w.id !== workflowId)),
  run_api_workflow: ({ request: r }) => {
    const w = data.apiWorkflows.find((w) => w.id === r.workflowId);
    if (!w) throw Error('Select a workflow');
    let s = data.sessions.find(
      (s) =>
        s.profileId === w.profileId &&
        s.state === 'ready' &&
        data.accounts.some((a) => a.isActive && s.accountId === a.id),
    );
    const coldStarted = !s;
    if (!s) {
      if (!r.autoWake || !data.settings.autoWakeOnApiCall) throw Error('Automatic wake is disabled');
      s = launch(w.profileId);
      ready(s);
    }
    return {
      jobId: crypto.randomUUID(),
      promptId: crypto.randomUUID(),
      sessionName: s.name,
      coldStarted,
      status: 'completed',
      outputs: [s.driveRootPath + '/output/sandbox.' + (w.outputKind === 'video' ? 'mp4' : 'png')],
      driveSequencePath: r.sequenceName ? s.driveRootPath + '/sequences/' + r.sequenceName : null,
      elapsedSeconds: 2.8,
      error: null,
    };
  },
  inspect_gguf_repo: ({ repoId, targetVramMib }) => inspect(repoId, targetVramMib),
  deploy_gguf_model: ({ repoId, quant, localPort, accelerator, minVramGb }) => {
    const base = 'http://127.0.0.1:' + localPort;
    const deployment = {
      id: crypto.randomUUID(),
      sessionName: 'llama-sandbox-' + accelerator,
      repoId,
      chosenQuant: quant,
      ctxSize: 8192,
      localPort,
      apiBaseUrl: base + '/v1',
      webChatUrl: base,
      status: 'ready',
      logs: ['Sandbox deployment · ' + minVramGb + ' GB'],
    };
    data.llamaDeployments.unshift(deployment);
    return deployment;
  },
  chat_completion_proxy: ({ messages }) => {
    const prompt = messages[messages.length - 1]?.content;
    return (
      (data.settings.locale === 'en'
        ? 'Sandbox reply'
        : data.settings.locale === 'zh-TW'
          ? '沙盒回覆'
          : '沙盒回复') +
      ' · ' +
      prompt
    );
  },
};
export function simulate<K extends Command>(key: K, args: Args<K>) {
  try {
    const stored = localStorage.getItem(storageKey);
    if (stored) data = JSON.parse(stored);
  } catch {
    /* In-memory preview */
  }
  const value = handlers[key](args);
  persist();
  return structuredClone(value);
}
