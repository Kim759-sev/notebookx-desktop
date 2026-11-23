pub mod colab;
pub mod comfy;
pub mod gateway;
pub mod plugins;
pub mod state;
pub mod types;

use state::{AppCore, ModelDownloadInput};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use types::{
    AcceleratorKind, AppSettings, ColabSession, DashboardSnapshot, EnvironmentStatus,
    GgufRepoInspection, GoogleAccount, LlamaDeployment, PortTunnel, SavedApiWorkflow,
    WorkflowProfile, WorkflowRunRequest, WorkflowRunResult, WorkflowScanResult, ModelDownloadTask,
};

#[tauri::command]
async fn get_dashboard_snapshot(
    core: State<'_, Arc<AppCore>>,
) -> Result<DashboardSnapshot, String> {
    Ok(core.snapshot().await)
}

#[tauri::command]
async fn check_environment(core: State<'_, Arc<AppCore>>) -> Result<EnvironmentStatus, String> {
    Ok(core.check_environment().await)
}

#[tauri::command]
async fn install_colab_cli(core: State<'_, Arc<AppCore>>) -> Result<EnvironmentStatus, String> {
    core.install_colab_cli().await
}

#[tauri::command]
async fn update_settings(
    core: State<'_, Arc<AppCore>>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    core.update_settings(settings).await
}

#[tauri::command]
async fn save_secret(
    core: State<'_, Arc<AppCore>>,
    name: String,
    value: String,
) -> Result<AppSettings, String> {
    core.save_secret(&name, &value).await
}

#[tauri::command]
async fn start_account_login(core: State<'_, Arc<AppCore>>) -> Result<String, String> {
    core.start_account_login().await
}

#[tauri::command]
async fn complete_account_login(
    core: State<'_, Arc<AppCore>>,
    auth_code: Option<String>,
) -> Result<GoogleAccount, String> {
    core.complete_account_login(auth_code).await
}

#[tauri::command]
async fn adopt_cli_account(core: State<'_, Arc<AppCore>>) -> Result<GoogleAccount, String> {
    core.adopt_cli_account().await
}

#[tauri::command]
async fn switch_account(
    core: State<'_, Arc<AppCore>>,
    account_id: String,
) -> Result<Vec<GoogleAccount>, String> {
    core.switch_account(&account_id).await
}

#[tauri::command]
async fn remove_account(
    core: State<'_, Arc<AppCore>>,
    account_id: String,
) -> Result<Vec<GoogleAccount>, String> {
    core.remove_account(&account_id).await
}

#[tauri::command]
async fn refresh_accounts_usage(
    core: State<'_, Arc<AppCore>>,
) -> Result<Vec<GoogleAccount>, String> {
    core.refresh_accounts_usage().await
}

#[tauri::command]
async fn save_workflow_profile(
    core: State<'_, Arc<AppCore>>,
    profile: WorkflowProfile,
) -> Result<Vec<WorkflowProfile>, String> {
    core.save_workflow_profile(profile).await
}

#[tauri::command]
async fn delete_workflow_profile(
    core: State<'_, Arc<AppCore>>,
    profile_id: String,
) -> Result<Vec<WorkflowProfile>, String> {
    core.delete_workflow_profile(&profile_id).await
}

#[tauri::command]
async fn launch_profile_session(
    core: State<'_, Arc<AppCore>>,
    profile_id: String,
) -> Result<ColabSession, String> {
    core.launch_profile_session(&profile_id).await
}

#[tauri::command]
async fn stop_session(
    core: State<'_, Arc<AppCore>>,
    session_name: String,
    force: bool,
) -> Result<ColabSession, String> {
    core.stop_session(&session_name, force).await
}

#[tauri::command]
async fn extend_session_deadline(
    core: State<'_, Arc<AppCore>>,
    session_name: String,
    add_minutes: u32,
) -> Result<ColabSession, String> {
    core.extend_session_deadline(&session_name, add_minutes).await
}

#[tauri::command]
async fn save_comfy_environment_snapshot(
    core: State<'_, Arc<AppCore>>,
    session_name: String,
) -> Result<String, String> {
    core.save_comfy_environment_snapshot(&session_name).await
}

#[tauri::command]
async fn open_comfy_desktop_window(
    app: AppHandle,
    url: String,
    title: Option<String>,
) -> Result<(), String> {
    let parsed: tauri::Url = url
        .parse()
        .map_err(|e| format!("Invalid ComfyUI URL '{url}': {e}"))?;

    if let Some(existing) = app.get_webview_window("comfyui-desktop") {
        let _ = existing.navigate(parsed);
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }

    let win_title = title.unwrap_or_else(|| "NotebookX · 云端桌面版 ComfyUI".to_string());
    WebviewWindowBuilder::new(&app, "comfyui-desktop", WebviewUrl::External(parsed))
        .title(&win_title)
        .inner_size(1440.0, 920.0)
        .min_inner_size(960.0, 640.0)
        .center()
        .resizable(true)
        .build()
        .map_err(|e| format!("Failed to create ComfyUI desktop window: {e}"))?;

    Ok(())
}

#[tauri::command]
async fn sync_dramasmith_endpoint(
    core: State<'_, Arc<AppCore>>,
    endpoint_url: String,
) -> Result<String, String> {
    core.sync_dramasmith_endpoint(&endpoint_url).await
}

#[tauri::command]
async fn create_port_tunnel(
    core: State<'_, Arc<AppCore>>,
    session_name: String,
    remote_port: u16,
    local_port: u16,
    label: String,
    health_path: Option<String>,
) -> Result<PortTunnel, String> {
    core.create_port_tunnel(&session_name, remote_port, local_port, &label, health_path)
        .await
}

#[tauri::command]
async fn stop_port_tunnel(core: State<'_, Arc<AppCore>>, tunnel_id: String) -> Result<(), String> {
    core.stop_port_tunnel(&tunnel_id).await
}

#[tauri::command]
async fn scan_workflow_models(
    core: State<'_, Arc<AppCore>>,
    workflow_name: String,
    workflow_json: String,
) -> Result<WorkflowScanResult, String> {
    core.scan_workflow_models(&workflow_name, &workflow_json).await
}

#[tauri::command]
async fn enqueue_model_downloads(
    core: State<'_, Arc<AppCore>>,
    items: Vec<ModelDownloadInput>,
) -> Result<Vec<ModelDownloadTask>, String> {
    core.enqueue_model_downloads(items).await
}

#[tauri::command]
async fn start_model_downloads(
    core: State<'_, Arc<AppCore>>,
    use_cpu_mode: bool,
    session_name: Option<String>,
) -> Result<Vec<ModelDownloadTask>, String> {
    core.start_model_downloads(use_cpu_mode, session_name).await
}

#[tauri::command]
async fn remove_model_download(
    core: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<Vec<ModelDownloadTask>, String> {
    core.remove_model_download(&task_id).await
}

#[tauri::command]
async fn save_api_workflow(
    core: State<'_, Arc<AppCore>>,
    workflow: SavedApiWorkflow,
) -> Result<Vec<SavedApiWorkflow>, String> {
    core.save_api_workflow(workflow).await
}

#[tauri::command]
async fn delete_api_workflow(
    core: State<'_, Arc<AppCore>>,
    workflow_id: String,
) -> Result<Vec<SavedApiWorkflow>, String> {
    core.delete_api_workflow(&workflow_id).await
}

#[tauri::command]
async fn run_api_workflow(
    core: State<'_, Arc<AppCore>>,
    request: WorkflowRunRequest,
) -> Result<WorkflowRunResult, String> {
    core.run_api_workflow(request).await
}

#[tauri::command]
async fn inspect_gguf_repo(
    core: State<'_, Arc<AppCore>>,
    repo_id: String,
    target_vram_mib: u32,
) -> Result<GgufRepoInspection, String> {
    core.inspect_gguf_repo(&repo_id, target_vram_mib).await
}

#[tauri::command]
async fn deploy_gguf_model(
    core: State<'_, Arc<AppCore>>,
    repo_id: String,
    accelerator: AcceleratorKind,
    min_vram_gb: u32,
    quant: Option<String>,
    local_port: u16,
) -> Result<LlamaDeployment, String> {
    core.deploy_gguf_model(&repo_id, accelerator, min_vram_gb, quant, local_port)
        .await
}

#[tauri::command]
async fn chat_completion_proxy(
    core: State<'_, Arc<AppCore>>,
    api_base_url: String,
    messages: Vec<serde_json::Value>,
    model: Option<String>,
) -> Result<String, String> {
    core.chat_completion_proxy(&api_base_url, messages, model).await
}

fn setup_system_tray(app: &tauri::App, core: Arc<AppCore>) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "打开 NotebookX 控制台", true, None::<&str>)?;
    let comfy_item = MenuItem::with_id(
        app,
        "open_comfy",
        "打开桌面版 ComfyUI (127.0.0.1:8188)",
        true,
        None::<&str>,
    )?;
    let stop_all_item = MenuItem::with_id(
        app,
        "stop_all",
        "安全备份并停止所有云端扣费",
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "彻底退出 NotebookX", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &comfy_item, &stop_all_item, &quit_item])?;

    let mut tray_builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("NotebookX Desktop · 云端 ComfyUI 与算力管家")
        .on_menu_event(move |app_handle, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "open_comfy" => {
                let handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = open_comfy_desktop_window(
                        handle,
                        "http://127.0.0.1:8188".to_string(),
                        None,
                    )
                    .await;
                });
            }
            "stop_all" => {
                let c = core.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = c.stop_all_active_sessions(false).await;
                });
            }
            "quit" => {
                app_handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app_handle = tray.app_handle();
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    let _ = tray_builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let core = Arc::new(AppCore::initialize());

    if std::env::args().any(|a| a == "--headless") {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async move {
            core.start_background_services().await;
            let _ = tokio::signal::ctrl_c().await;
        });
        return;
    }

    tauri::Builder::default()
        .manage(core.clone())
        .setup(move |app| {
            if let Err(err) = setup_system_tray(app, core.clone()) {
                tracing::warn!("System tray setup skipped: {err}");
            }

            let startup_core = core.clone();
            tauri::async_runtime::spawn(async move {
                startup_core.start_background_services().await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    let core = window.state::<Arc<AppCore>>().inner().clone();
                    if core.should_minimize_to_tray_sync() {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_dashboard_snapshot,
            check_environment,
            install_colab_cli,
            update_settings,
            save_secret,
            start_account_login,
            complete_account_login,
            adopt_cli_account,
            switch_account,
            remove_account,
            refresh_accounts_usage,
            save_workflow_profile,
            delete_workflow_profile,
            launch_profile_session,
            stop_session,
            extend_session_deadline,
            save_comfy_environment_snapshot,
            open_comfy_desktop_window,
            sync_dramasmith_endpoint,
            create_port_tunnel,
            stop_port_tunnel,
            scan_workflow_models,
            enqueue_model_downloads,
            start_model_downloads,
            remove_model_download,
            save_api_workflow,
            delete_api_workflow,
            run_api_workflow,
            inspect_gguf_repo,
            deploy_gguf_model,
            chat_completion_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NotebookX Desktop");
}

#[cfg(test)]
mod gateway_integration_tests {
    use super::*;
    use crate::types::BackendMode;
    use std::collections::HashMap;

    #[tokio::test]
    async fn gateway_auto_wake_and_h3_continuity_e2e() {
        let core = Arc::new(AppCore::initialize());
        let mut settings = core.snapshot().await.settings;
        settings.backendMode = BackendMode::Mock;
        let _ = core.update_settings(settings).await.unwrap();

        let addr = crate::gateway::start_local_gateway(core.clone(), 0)
            .await
            .unwrap();
        let client = reqwest::Client::new();

        // 1. Health before launch: connected == false
        let h1: serde_json::Value = client
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(h1["ok"], true);

        // 2. Run first segment ('start') via HTTP gateway — triggers cold-start auto-wake!
        let mut params = HashMap::new();
        params.insert(
            "prompt".to_string(),
            serde_json::json!("Shot 1: rain duel at midnight"),
        );
        let r1: WorkflowRunResult = client
            .post(format!("http://{addr}/api/v1/comfy/run"))
            .json(&WorkflowRunRequest {
                workflowId: Some("colab-bf16-continuity".to_string()),
                profileId: None,
                rawPromptGraph: None,
                params,
                continuityMode: Some("start".to_string()),
                parentTaskId: None,
                sequenceName: Some("test_seq_01".to_string()),
                autoWake: true,
                waitForCompletion: true,
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(r1.status, "completed");
        assert!(r1.coldStarted);

        // 3. Run second segment ('continue') referencing r1.jobId — warm reuse + H3 timeline append!
        let mut params2 = HashMap::new();
        params2.insert(
            "prompt".to_string(),
            serde_json::json!("Shot 2: sword clash close-up"),
        );
        let r2: WorkflowRunResult = client
            .post(format!("http://{addr}/api/v1/comfy/run"))
            .json(&WorkflowRunRequest {
                workflowId: Some("colab-bf16-continuity".to_string()),
                profileId: None,
                rawPromptGraph: None,
                params: params2,
                continuityMode: Some("continue".to_string()),
                parentTaskId: Some(r1.jobId),
                sequenceName: Some("test_seq_01".to_string()),
                autoWake: true,
                waitForCompletion: true,
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(r2.status, "completed");
        assert!(!r2.coldStarted);
    }
}
