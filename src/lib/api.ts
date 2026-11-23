import { invoke } from '@tauri-apps/api/core';
import type {
  AcceleratorKind,
  AppSettings,
  ColabSession,
  DashboardSnapshot,
  EnvironmentStatus,
  GgufRepoInspection,
  GoogleAccount,
  LlamaDeployment,
  ModelCategory,
  ModelDownloadTask,
  PortTunnel,
  SavedApiWorkflow,
  WorkflowProfile,
  WorkflowRunRequest,
  WorkflowRunResult,
  WorkflowScanResult,
} from './types';
import { simulate } from './sandbox';

export interface DownloadItem {
  filename: string;
  sourceUrl: string;
  category: ModelCategory;
  targetDriveDir: string;
}
export interface Message {
  role: 'system' | 'user' | 'assistant';
  content: string;
}
interface Commands {
  get_dashboard_snapshot: [undefined, DashboardSnapshot];
  check_environment: [undefined, EnvironmentStatus];
  install_colab_cli: [undefined, EnvironmentStatus];
  update_settings: [{ settings: AppSettings }, AppSettings];
  save_secret: [{ name: string; value: string }, AppSettings];
  start_account_login: [undefined, string];
  complete_account_login: [{ authCode: string }, GoogleAccount];
  adopt_cli_account: [undefined, GoogleAccount];
  switch_account: [{ accountId: string }, GoogleAccount[]];
  remove_account: [{ accountId: string }, GoogleAccount[]];
  refresh_accounts_usage: [undefined, GoogleAccount[]];
  save_workflow_profile: [{ profile: WorkflowProfile }, WorkflowProfile[]];
  delete_workflow_profile: [{ profileId: string }, WorkflowProfile[]];
  launch_profile_session: [{ profileId: string }, ColabSession];
  stop_session: [{ sessionName: string; force: boolean }, ColabSession];
  extend_session_deadline: [{ sessionName: string; addMinutes: number }, ColabSession];
  save_comfy_environment_snapshot: [{ sessionName: string }, string];
  open_comfy_desktop_window: [{ url: string; title: string }, void];
  sync_dramasmith_endpoint: [{ endpointUrl: string }, string];
  create_port_tunnel: [
    {
      sessionName: string;
      remotePort: number;
      localPort: number;
      label: string;
      healthPath: string | null;
    },
    PortTunnel,
  ];
  stop_port_tunnel: [{ tunnelId: string }, void];
  scan_workflow_models: [{ workflowName: string; workflowJson: string }, WorkflowScanResult];
  enqueue_model_downloads: [{ items: DownloadItem[] }, ModelDownloadTask[]];
  start_model_downloads: [{ useCpuMode: boolean; sessionName: string | null }, ModelDownloadTask[]];
  remove_model_download: [{ taskId: string }, ModelDownloadTask[]];
  save_api_workflow: [{ workflow: SavedApiWorkflow }, SavedApiWorkflow[]];
  delete_api_workflow: [{ workflowId: string }, SavedApiWorkflow[]];
  run_api_workflow: [{ request: WorkflowRunRequest }, WorkflowRunResult];
  inspect_gguf_repo: [{ repoId: string; targetVramMib: number }, GgufRepoInspection];
  deploy_gguf_model: [
    {
      repoId: string;
      accelerator: AcceleratorKind;
      minVramGb: number;
      quant: string;
      localPort: number;
    },
    LlamaDeployment,
  ];
  chat_completion_proxy: [{ apiBaseUrl: string; messages: Message[]; model: string }, string];
}
export type Command = keyof Commands;
export type Args<K extends Command> = Commands[K][0];
export type Result<K extends Command> = Commands[K][1];
export function isDesktop() {
  return (
    '__TAURI_INTERNALS__' in window &&
    Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
}
export async function api<K extends Command>(
  command: K,
  ...args: Args<K> extends undefined ? [] : [Args<K>]
): Promise<Commands[K][1]> {
  if (isDesktop()) return invoke<Commands[K][1]>(command, args[0]);
  return simulate(command, args[0]);
}
