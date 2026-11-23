#![allow(non_snake_case)]

use crate::types::*;
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadInput {
    pub filename: String,
    pub sourceUrl: String,
    pub category: ModelCategory,
    pub targetDriveDir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedStore {
    settings: AppSettings,
    accounts: Vec<GoogleAccount>,
    profiles: Vec<WorkflowProfile>,
    sessions: Vec<ColabSession>,
    driveModels: Vec<DriveModelItem>,
    downloads: Vec<ModelDownloadTask>,
    apiWorkflows: Vec<SavedApiWorkflow>,
    llamaDeployments: Vec<LlamaDeployment>,
    #[serde(default)]
    completedGraphs: HashMap<String, serde_json::Value>,
}

pub struct AppCore {
    pub data_dir: PathBuf,
    pub http_client: reqwest::Client,
    store: RwLock<PersistedStore>,
    tunnels: RwLock<Vec<PortTunnel>>,
    tunnel_stops: Mutex<HashMap<String, crate::colab::tunnel::TunnelHandle>>,
    gateway_bound_url: RwLock<Option<String>>,
    last_activity_epoch: RwLock<i64>,
    minimize_to_tray_flag: AtomicBool,
}

impl AppCore {
    pub fn initialize() -> Self {
        let data_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("notebookx-desktop");
        let _ = std::fs::create_dir_all(&data_dir);

        let store_path = data_dir.join("store.json");
        let loaded: Option<PersistedStore> = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok());

        let mut initial_store = loaded.unwrap_or_else(|| {
            let mut settings = AppSettings::default();
            settings.backendMode = BackendMode::Real;
            PersistedStore {
                settings,
                accounts: Vec::new(),
                profiles: crate::comfy::default_workflow_profiles(),
                sessions: Vec::new(),
                driveModels: Vec::new(),
                downloads: Vec::new(),
                apiWorkflows: crate::comfy::default_saved_api_workflows(),
                llamaDeployments: Vec::new(),
                completedGraphs: HashMap::new(),
            }
        });

        // Always start in Real Google Colab mode and purge any leftover demo/mock entries
        initial_store.settings.backendMode = BackendMode::Real;
        initial_store.accounts.retain(|a| {
            !a.id.starts_with("demo-")
                && !a.id.starts_with("mock-")
                && a.email != "studio.ultra20x@gmail.com"
                && a.email != "backup.render@gmail.com"
        });
        initial_store.sessions.retain(|s| {
            !s.accountId.starts_with("demo-")
                && !s.accountId.starts_with("mock-")
                && !s.endpoint.starts_with("colab-")
        });
        initial_store
            .driveModels
            .retain(|m| !(m.sizeBytes == 0 && m.updatedAt.is_none()));
        initial_store
            .llamaDeployments
            .retain(|d| !d.id.starts_with("llama-") || !d.logs.iter().any(|l| l.contains("已根据")));
        if let Ok(raw) = serde_json::to_string_pretty(&initial_store) {
            let _ = std::fs::write(&store_path, raw);
        }

        let minimize = initial_store.settings.minimizeToTray;
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            data_dir,
            http_client,
            store: RwLock::new(initial_store),
            tunnels: RwLock::new(Vec::new()),
            tunnel_stops: Mutex::new(HashMap::new()),
            gateway_bound_url: RwLock::new(None),
            last_activity_epoch: RwLock::new(Utc::now().timestamp()),
            minimize_to_tray_flag: AtomicBool::new(minimize),
        }
    }

    pub fn should_minimize_to_tray_sync(&self) -> bool {
        self.minimize_to_tray_flag.load(Ordering::Relaxed)
    }

    async fn persist(&self) {
        let guard = self.store.read().await;
        if let Ok(raw) = serde_json::to_string_pretty(&*guard) {
            let _ = std::fs::write(self.data_dir.join("store.json"), raw);
        }
    }

    pub async fn start_background_services(self: &Arc<Self>) {
        let port = { self.store.read().await.settings.gatewayPort };
        match crate::gateway::start_local_gateway(self.clone(), port).await {
            Ok(addr) => {
                let url = format!("http://{addr}");
                tracing::info!("Local Serverless Workflow API Gateway listening on {url}");
                *self.gateway_bound_url.write().await = Some(url);
            }
            Err(err) => {
                tracing::warn!("Could not bind preferred gateway port {port}: {err}");
                if let Ok(addr) = crate::gateway::start_local_gateway(self.clone(), 0).await {
                    let url = format!("http://{addr}");
                    *self.gateway_bound_url.write().await = Some(url);
                }
            }
        }

        // Refresh secret flags on startup in Real mode
        let _ = self.refresh_secret_flags().await;

        // Spawn periodic cost-guard & ComfyUI queue monitor loop
        let guardian = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(20));
            loop {
                interval.tick().await;
                guardian.guardian_tick().await;
            }
        });
    }

    async fn guardian_tick(&self) {
        let (mode, default_guard, active_sessions) = {
            let st = self.store.read().await;
            let active: Vec<ColabSession> = st
                .sessions
                .iter()
                .filter(|s| {
                    matches!(
                        s.state,
                        SessionLifecycle::Ready
                            | SessionLifecycle::Busy
                            | SessionLifecycle::Bootstrapping
                    )
                })
                .cloned()
                .collect();
            (st.settings.backendMode, st.settings.defaultCostGuard.clone(), active)
        };

        if active_sessions.is_empty() {
            return;
        }

        for sess in active_sessions {
            // Check local ComfyUI queue if ready
            let mut queue_running = 0u32;
            let mut queue_pending = 0u32;
            if sess.comfyReady && mode == BackendMode::Real {
                if let Some(url) = &sess.comfyLocalUrl {
                    if let Ok(resp) = self
                        .http_client
                        .get(format!("{url}/queue"))
                        .timeout(std::time::Duration::from_secs(4))
                        .send()
                        .await
                    {
                        if let Ok(val) = resp.json::<serde_json::Value>().await {
                            queue_running = val
                                .get("queue_running")
                                .and_then(|a| a.as_array())
                                .map(|a| a.len() as u32)
                                .unwrap_or(0);
                            queue_pending = val
                                .get("queue_pending")
                                .and_then(|a| a.as_array())
                                .map(|a| a.len() as u32)
                                .unwrap_or(0);
                        }
                    }
                }
            }

            if queue_running > 0 || queue_pending > 0 {
                self.touch_activity().await;
            }

            let now_ts = Utc::now().timestamp();
            let last_act = *self.last_activity_epoch.read().await;
            let idle_elapsed_min = ((now_ts - last_act).max(0) / 60) as u32;

            let mut should_auto_stop = false;
            let mut stop_reason = String::new();

            if let Some(idle_limit) = default_guard.idleStopMinutes {
                if queue_running == 0 && queue_pending == 0 && idle_elapsed_min >= idle_limit {
                    should_auto_stop = true;
                    stop_reason = format!("已达闲置自动关机阈值 ({idle_limit} 分钟)");
                }
            }

            {
                let mut st = self.store.write().await;
                let burn_rate = st
                    .accounts
                    .iter()
                    .find(|a| a.isActive)
                    .and_then(|a| a.burnRateHourly)
                    .unwrap_or_else(|| match sess.accelerator {
                        AcceleratorKind::G4 | AcceleratorKind::H100 => 11.5,
                        AcceleratorKind::A100 => 7.2,
                        AcceleratorKind::L4 => 2.3,
                        AcceleratorKind::T4 => 1.2,
                        _ => 0.1,
                    });

                if let Some(target) = st.sessions.iter_mut().find(|s| s.name == sess.name) {
                    target.queueRunning = queue_running;
                    target.queuePending = queue_pending;
                    if queue_running > 0 {
                        target.state = SessionLifecycle::Busy;
                    } else if target.comfyReady && target.state == SessionLifecycle::Busy {
                        target.state = SessionLifecycle::Ready;
                    }
                    target.spentCcu += burn_rate * (20.0 / 3600.0);
                    if let Some(idle_limit) = default_guard.idleStopMinutes {
                        target.idleMinutesLeft = Some(idle_limit.saturating_sub(idle_elapsed_min));
                    }
                    if let Some(cap) = default_guard.spendingCapCcu {
                        if target.spentCcu >= cap {
                            should_auto_stop = true;
                            stop_reason = format!("已达单次消耗上限 ({cap:.1} CCU)");
                        }
                    }
                }

                if let Some(floor) = default_guard.balanceFloorCcu {
                    if let Some(acc) = st.accounts.iter().find(|a| a.isActive) {
                        if let Some(bal) = acc.balanceCcu {
                            if bal <= floor && bal > 0.0 {
                                should_auto_stop = true;
                                stop_reason = format!("已触发积分余额底线保护 (剩余 {bal:.1} <= {floor:.1} CCU)");
                            }
                        }
                    }
                }
            }

            if should_auto_stop {
                tracing::info!("Cost guard stopping session {}: {}", sess.name, stop_reason);
                let _ = self.stop_session(&sess.name, false).await;
            }
        }
    }

    pub async fn touch_activity(&self) {
        *self.last_activity_epoch.write().await = Utc::now().timestamp();
    }

    pub async fn check_environment(&self) -> EnvironmentStatus {
        let (mode, gw_port, ds_url) = {
            let st = self.store.read().await;
            (
                st.settings.backendMode,
                st.settings.gatewayPort,
                st.settings.dramaSmithApiUrl.clone(),
            )
        };
        let gw_bound = self.gateway_bound_url.read().await.clone();
        let gateway_url = gw_bound
            .clone()
            .unwrap_or_else(|| format!("http://127.0.0.1:{gw_port}"));
        let ds_ok = crate::comfy::check_dramasmith_reachable(&self.http_client, &ds_url).await;

        let mut env = crate::colab::toolchain::detect_environment(
            mode,
            gw_bound.is_some(),
            gateway_url,
            ds_ok,
            ds_url,
        )
        .await;
        env.backendMode = mode;
        env
    }

    pub async fn snapshot(&self) -> DashboardSnapshot {
        let env = self.check_environment().await;
        let plugins = crate::plugins::load_all_plugins(&self.data_dir);
        let st = self.store.read().await;
        let mut tunnels = self.tunnels.read().await.clone();
        {
            let handles = self.tunnel_stops.lock().await;
            for tun in tunnels.iter_mut() {
                if let Some(handle) = handles.get(&tun.id) {
                    let live = handle.snapshot();
                    tun.status = live.status;
                    tun.healthy = live.healthy;
                    tun.reconnectCount = live.reconnectCount;
                    tun.lastError = live.lastError;
                }
            }
        }

        let accounts = st.accounts.clone();

        DashboardSnapshot {
            env,
            settings: st.settings.clone(),
            accounts,
            profiles: st.profiles.clone(),
            sessions: st.sessions.clone(),
            tunnels,
            driveModels: st.driveModels.clone(),
            downloads: st.downloads.clone(),
            apiWorkflows: st.apiWorkflows.clone(),
            plugins,
            llamaDeployments: st.llamaDeployments.clone(),
        }
    }

    pub async fn install_colab_cli(&self) -> Result<EnvironmentStatus, String> {
        let mode = self.store.read().await.settings.backendMode;
        if mode == BackendMode::Real {
            crate::colab::toolchain::install_colab_cli().await?;
        }
        Ok(self.check_environment().await)
    }

    pub async fn update_settings(&self, mut next: AppSettings) -> Result<AppSettings, String> {
        self.minimize_to_tray_flag
            .store(next.minimizeToTray, Ordering::Relaxed);
        {
            let prev = &self.store.read().await.settings;
            next.hasHfToken = prev.hasHfToken;
            next.hasCivitaiToken = prev.hasCivitaiToken;
            next.hasRcloneConfig = prev.hasRcloneConfig;
        }
        self.store.write().await.settings = next.clone();
        self.persist().await;
        Ok(next)
    }

    async fn refresh_secret_flags(&self) -> Result<(), String> {
        let has_hf = crate::colab::accounts::secret_has(&self.data_dir, "hf_token")
            .await
            .unwrap_or(false);
        let has_civ = crate::colab::accounts::secret_has(&self.data_dir, "civitai_token")
            .await
            .unwrap_or(false);
        let has_rc = crate::colab::accounts::secret_has(&self.data_dir, "rclone_config")
            .await
            .unwrap_or(false);
        let mut st = self.store.write().await;
        st.settings.hasHfToken = has_hf;
        st.settings.hasCivitaiToken = has_civ;
        st.settings.hasRcloneConfig = has_rc;
        Ok(())
    }

    pub async fn save_secret(&self, name: &str, value: &str) -> Result<AppSettings, String> {
        if value.trim().is_empty() {
            crate::colab::accounts::secret_delete(&self.data_dir, name).await?;
        } else {
            crate::colab::accounts::secret_set(&self.data_dir, name, value.trim()).await?;
        }
        let _ = self.refresh_secret_flags().await;
        self.persist().await;
        Ok(self.store.read().await.settings.clone())
    }

    pub async fn start_account_login(&self) -> Result<String, String> {
        let mode = self.store.read().await.settings.backendMode;
        if mode == BackendMode::Mock {
            return Ok("https://accounts.google.com/o/oauth2/v2/auth?mock=notebookx".to_string());
        }
        let existing_ids: Vec<String> = self
            .store
            .read()
            .await
            .accounts
            .iter()
            .map(|a| a.id.clone())
            .collect();
        crate::colab::accounts::start_oauth_login(&self.data_dir, &existing_ids).await
    }

    pub async fn complete_account_login(
        &self,
        auth_code: Option<String>,
    ) -> Result<GoogleAccount, String> {
        let mode = self.store.read().await.settings.backendMode;
        if mode == BackendMode::Mock {
            let id = format!("mock-{}", &Uuid::new_v4().to_string()[..8]);
            let acc = GoogleAccount {
                id: id.clone(),
                email: format!("creator.{}@gmail.com", &id[5..9]),
                displayName: Some("Colab Pro+ 账号".to_string()),
                isActive: true,
                planTier: "PRO+".to_string(),
                balanceCcu: Some(1980.0),
                burnRateHourly: Some(0.0),
                hoursRemaining: None,
                activeAssignmentsCount: 0,
                lastUpdatedAt: Some(Utc::now().to_rfc3339()),
                usageStale: false,
            };
            let mut st = self.store.write().await;
            for a in st.accounts.iter_mut() {
                a.isActive = false;
            }
            st.accounts.push(acc.clone());
            drop(st);
            self.persist().await;
            return Ok(acc);
        }

        let new_id = format!("acc-{}", &Uuid::new_v4().to_string()[..8]);
        let mut acc =
            crate::colab::accounts::complete_oauth_login(&self.data_dir, &new_id, auth_code)
                .await?;
        acc.isActive = true;
        // Activate newly signed-in account and immediately fetch its live CO PRO+ CCU balance!
        let _ = crate::colab::accounts::use_saved_account(&self.data_dir, &new_id).await;
        if let Ok(usage) = crate::colab::accounts::fetch_account_usage(&self.data_dir, &new_id).await {
            acc.balanceCcu = Some(usage.balance_ccu);
            acc.burnRateHourly = Some(usage.burn_rate_hourly);
            acc.hoursRemaining = if usage.burn_rate_hourly > 0.01 {
                Some(usage.balance_ccu / usage.burn_rate_hourly)
            } else {
                None
            };
            acc.activeAssignmentsCount = usage.assignments_count;
            acc.planTier = usage.plan_tier;
            acc.lastUpdatedAt = Some(Utc::now().to_rfc3339());
            acc.usageStale = false;
        }
        {
            let mut st = self.store.write().await;
            for a in st.accounts.iter_mut() {
                a.isActive = false;
            }
            st.accounts.retain(|a| a.email != acc.email);
            st.accounts.push(acc.clone());
        }
        self.persist().await;
        Ok(acc)
    }

    pub async fn adopt_cli_account(&self) -> Result<GoogleAccount, String> {
        let mode = self.store.read().await.settings.backendMode;
        if mode == BackendMode::Mock {
            return self.complete_account_login(None).await;
        }
        let new_id = format!("acc-{}", &Uuid::new_v4().to_string()[..8]);
        let mut acc = crate::colab::accounts::adopt_active_cli_account(&self.data_dir, &new_id).await?;
        acc.isActive = true;
        if let Ok(usage) = crate::colab::accounts::fetch_account_usage(&self.data_dir, &new_id).await {
            acc.balanceCcu = Some(usage.balance_ccu);
            acc.burnRateHourly = Some(usage.burn_rate_hourly);
            acc.hoursRemaining = if usage.burn_rate_hourly > 0.01 {
                Some(usage.balance_ccu / usage.burn_rate_hourly)
            } else {
                None
            };
            acc.activeAssignmentsCount = usage.assignments_count;
            acc.planTier = usage.plan_tier;
            acc.lastUpdatedAt = Some(Utc::now().to_rfc3339());
            acc.usageStale = false;
        }
        {
            let mut st = self.store.write().await;
            for a in st.accounts.iter_mut() {
                a.isActive = false;
            }
            if let Some(existing) = st.accounts.iter_mut().find(|a| a.email == acc.email) {
                existing.isActive = true;
                existing.balanceCcu = acc.balanceCcu;
                existing.burnRateHourly = acc.burnRateHourly;
                existing.planTier = acc.planTier.clone();
                acc = existing.clone();
            } else {
                st.accounts.push(acc.clone());
            }
        }
        self.persist().await;
        Ok(acc)
    }

    pub async fn switch_account(&self, account_id: &str) -> Result<Vec<GoogleAccount>, String> {
        let mode = self.store.read().await.settings.backendMode;
        if mode == BackendMode::Real {
            crate::colab::accounts::use_saved_account(&self.data_dir, account_id).await?;
        }
        let mut st = self.store.write().await;
        for a in st.accounts.iter_mut() {
            a.isActive = a.id == account_id;
        }
        let out = st.accounts.clone();
        drop(st);
        self.persist().await;
        Ok(out)
    }

    pub async fn remove_account(&self, account_id: &str) -> Result<Vec<GoogleAccount>, String> {
        let mode = self.store.read().await.settings.backendMode;
        if mode == BackendMode::Real {
            let _ = crate::colab::accounts::remove_saved_account(&self.data_dir, account_id).await;
        }
        let mut st = self.store.write().await;
        st.accounts.retain(|a| a.id != account_id);
        if !st.accounts.iter().any(|a| a.isActive) {
            if let Some(first) = st.accounts.first_mut() {
                first.isActive = true;
            }
        }
        let out = st.accounts.clone();
        drop(st);
        self.persist().await;
        Ok(out)
    }

    pub async fn refresh_accounts_usage(&self) -> Result<Vec<GoogleAccount>, String> {
        let mode = self.store.read().await.settings.backendMode;
        if mode == BackendMode::Mock {
            return Ok(self.snapshot().await.accounts);
        }

        let accounts_snapshot = self.store.read().await.accounts.clone();
        let mut updated = Vec::with_capacity(accounts_snapshot.len());
        for mut acc in accounts_snapshot {
            match crate::colab::accounts::fetch_account_usage(&self.data_dir, &acc.id).await {
                Ok(usage) => {
                    acc.balanceCcu = Some(usage.balance_ccu);
                    acc.burnRateHourly = Some(usage.burn_rate_hourly);
                    acc.hoursRemaining = if usage.burn_rate_hourly > 0.01 {
                        Some(usage.balance_ccu / usage.burn_rate_hourly)
                    } else {
                        None
                    };
                    acc.activeAssignmentsCount = usage.assignments_count;
                    acc.planTier = usage.plan_tier;
                    acc.lastUpdatedAt = Some(Utc::now().to_rfc3339());
                    acc.usageStale = false;
                }
                Err(_) => {
                    acc.usageStale = true;
                }
            }
            updated.push(acc);
        }

        // Also reconcile real active assignments on active account to expose any browser-opened [?] orphans!
        if let Some(active_acc) = updated.iter().find(|a| a.isActive) {
            if let Ok(assignments) =
                crate::colab::accounts::list_active_assignments(&self.data_dir, &active_acc.id)
                    .await
            {
                let mut st = self.store.write().await;
                for asg in assignments {
                    let known = st.sessions.iter().any(|s| {
                        s.endpoint == asg.endpoint && s.state != SessionLifecycle::Stopped
                    });
                    if !known {
                        st.sessions.push(ColabSession {
                            id: format!("orphan-{}", &Uuid::new_v4().to_string()[..6]),
                            name: format!("[浏览器实例] {}", asg.endpoint),
                            accountId: active_acc.id.clone(),
                            accountEmail: active_acc.email.clone(),
                            endpoint: asg.endpoint,
                            accelerator: asg.accelerator,
                            gpuName: Some(format!("{:?} (浏览器创建)", asg.accelerator)),
                            vramTotalMib: Some(asg.accelerator.nominal_vram_mib()),
                            vramUsedMib: None,
                            highMem: asg.high_mem,
                            profileId: None,
                            profileName: Some("外部浏览器会话".to_string()),
                            state: SessionLifecycle::Orphan,
                            stages: Vec::new(),
                            driveMounted: false,
                            driveRootPath: None,
                            driveAuthUrl: None,
                            comfyLocalUrl: None,
                            managementLocalUrl: None,
                            cloudflareUrl: None,
                            comfyReady: false,
                            loadedRequiredNodes: Vec::new(),
                            missingRequiredNodes: Vec::new(),
                            queueRunning: 0,
                            queuePending: 0,
                            watchdogActive: false,
                            autoStopAt: None,
                            idleMinutesLeft: None,
                            spentCcu: 0.0,
                            startedAt: Utc::now().to_rfc3339(),
                            verifiedUnassigned: false,
                            lastError: None,
                            logs: vec!["检测到账号下存在浏览器开启的计费实例，可随时在此一键安全释放停止扣费。".to_string()],
                        });
                    }
                }
                st.accounts = updated.clone();
            } else {
                self.store.write().await.accounts = updated.clone();
            }
        } else {
            self.store.write().await.accounts = updated.clone();
        }

        self.persist().await;
        Ok(updated)
    }

    pub async fn save_workflow_profile(
        &self,
        profile: WorkflowProfile,
    ) -> Result<Vec<WorkflowProfile>, String> {
        let mut st = self.store.write().await;
        if let Some(existing) = st.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            st.profiles.push(profile);
        }
        let out = st.profiles.clone();
        drop(st);
        self.persist().await;
        Ok(out)
    }

    pub async fn delete_workflow_profile(
        &self,
        profile_id: &str,
    ) -> Result<Vec<WorkflowProfile>, String> {
        let mut st = self.store.write().await;
        st.profiles.retain(|p| p.id != profile_id);
        let out = st.profiles.clone();
        drop(st);
        self.persist().await;
        Ok(out)
    }

    pub async fn ensure_or_launch_profile(&self, profile_id: &str) -> Result<ColabSession, String> {
        {
            let st = self.store.read().await;
            if let Some(ready) = st.sessions.iter().find(|s| {
                s.comfyReady
                    && matches!(s.state, SessionLifecycle::Ready | SessionLifecycle::Busy)
            }) {
                return Ok(ready.clone());
            }
        }
        self.launch_profile_session(profile_id).await
    }

    pub async fn launch_profile_session(&self, profile_id: &str) -> Result<ColabSession, String> {
        self.touch_activity().await;
        let (mode, profile, active_acc, ds_url, auto_sync_ds) = {
            let st = self.store.read().await;
            let prof = st
                .profiles
                .iter()
                .find(|p| p.id == profile_id)
                .cloned()
                .ok_or_else(|| format!("未找到工作流启动档位 '{profile_id}'"))?;
            let acc = st.accounts.iter().find(|a| a.isActive).cloned();
            (
                st.settings.backendMode,
                prof,
                acc,
                st.settings.dramaSmithApiUrl.clone(),
                st.settings.autoSyncDramaSmith,
            )
        };

        let session_short = &Uuid::new_v4().to_string()[..6];
        let session_name = format!("nbx-{}-{session_short}", profile.accelerator_slug());
        let auto_stop_at = profile
            .costGuard
            .autoStopMinutes
            .map(|m| (Utc::now() + ChronoDuration::minutes(m as i64)).to_rfc3339());

        let initial_stages = vec![
            StartupStage {
                id: "provision".to_string(),
                label: format!("分配 {:?} 算力与显存校验", profile.accelerator),
                status: StageStatus::Running,
                detail: Some("正在请求 Google Colab 分配实例...".to_string()),
            },
            StartupStage {
                id: "mount_drive".to_string(),
                label: "挂载 Google Drive 模型与配置库".to_string(),
                status: StageStatus::Pending,
                detail: Some(profile.driveRootPath.clone()),
            },
            StartupStage {
                id: "restore_env".to_string(),
                label: "恢复 ComfyUI 插件快照与模型映射".to_string(),
                status: StageStatus::Pending,
                detail: None,
            },
            StartupStage {
                id: "tunnel".to_string(),
                label: format!("打通本地 127.0.0.1:{} 直连隧道", profile.comfyPort),
                status: StageStatus::Pending,
                detail: None,
            },
            StartupStage {
                id: "readiness".to_string(),
                label: "出片级深度自检 (节点 / 显存 / 队列)".to_string(),
                status: StageStatus::Pending,
                detail: None,
            },
        ];

        let (account_id, account_email) = active_acc
            .as_ref()
            .map(|a| (a.id.clone(), a.email.clone()))
            .unwrap_or_else(|| ("demo-pro-plus".to_string(), "studio.ultra20x@gmail.com".to_string()));

        let mut session = ColabSession {
            id: Uuid::new_v4().to_string(),
            name: session_name.clone(),
            accountId: account_id.clone(),
            accountEmail: account_email,
            endpoint: format!("colab-{session_short}"),
            accelerator: profile.accelerator,
            gpuName: None,
            vramTotalMib: None,
            vramUsedMib: None,
            highMem: profile.highMem,
            profileId: Some(profile.id.clone()),
            profileName: Some(profile.name.clone()),
            state: SessionLifecycle::Provisioning,
            stages: initial_stages,
            driveMounted: false,
            driveRootPath: Some(profile.driveRootPath.clone()),
            driveAuthUrl: None,
            comfyLocalUrl: None,
            managementLocalUrl: None,
            cloudflareUrl: None,
            comfyReady: false,
            loadedRequiredNodes: Vec::new(),
            missingRequiredNodes: Vec::new(),
            queueRunning: 0,
            queuePending: 0,
            watchdogActive: false,
            autoStopAt: auto_stop_at,
            idleMinutesLeft: profile.costGuard.idleStopMinutes,
            spentCcu: 0.0,
            startedAt: Utc::now().to_rfc3339(),
            verifiedUnassigned: false,
            lastError: None,
            logs: vec![format!(
                "[NotebookX] 启动档位「{}」({:?})",
                profile.name, profile.accelerator
            )],
        };

        {
            let mut st = self.store.write().await;
            st.sessions
                .retain(|s| s.state != SessionLifecycle::Stopped);
            st.sessions.insert(0, session.clone());
        }

        if mode == BackendMode::Mock {
            // Simulate fast realistic 5-stage provisioning for UI testing
            let gpu_label = match profile.accelerator {
                AcceleratorKind::G4 => "NVIDIA RTX PRO 6000 Blackwell Server Edition (96 GB)",
                AcceleratorKind::A100 => {
                    if profile.minVramGb >= 70 {
                        "NVIDIA A100-SXM4-80GB (第 2 次抽卡命中 80GB)"
                    } else {
                        "NVIDIA A100-SXM4-40GB"
                    }
                }
                AcceleratorKind::H100 => "NVIDIA H100 80GB HBM3",
                AcceleratorKind::L4 => "NVIDIA L4 (24 GB)",
                AcceleratorKind::T4 => "NVIDIA Tesla T4 (15 GB)",
                AcceleratorKind::Cpu => "Google Colab High-Throughput CPU (零显卡积分)",
                AcceleratorKind::V5e1 => "Cloud TPU v5e-1",
                AcceleratorKind::V6e1 => "Cloud TPU v6e-1",
            };
            let vram = if profile.accelerator == AcceleratorKind::A100 && profile.minVramGb >= 70 {
                81_920
            } else {
                profile.accelerator.nominal_vram_mib()
            };

            session.gpuName = Some(gpu_label.to_string());
            session.vramTotalMib = Some(vram);
            session.vramUsedMib = Some(if vram > 0 { 14_200.min(vram / 2) } else { 0 });
            session.driveMounted = profile.driveMountMode != DriveMountMode::Disabled;
            session.watchdogActive = true;
            session.comfyLocalUrl = Some(format!("http://127.0.0.1:{}", profile.comfyPort));
            if let Some(mp) = profile.managementPort {
                session.managementLocalUrl = Some(format!("http://127.0.0.1:{mp}"));
            }
            session.comfyReady = profile.startupMode != ProfileStartupMode::CpuPreDownload;
            session.loadedRequiredNodes = profile.requiredNodes.clone();
            session.state = SessionLifecycle::Ready;
            for stg in session.stages.iter_mut() {
                stg.status = StageStatus::Completed;
                if stg.id == "provision" {
                    stg.detail = Some(gpu_label.to_string());
                } else if stg.id == "readiness" {
                    stg.detail = Some(if session.loadedRequiredNodes.is_empty() {
                        "ComfyUI /system_stats 与 /queue 就绪".to_string()
                    } else {
                        format!(
                            "已就绪 · 节点已加载: {}",
                            session.loadedRequiredNodes.join(", ")
                        )
                    });
                }
            }
            session.logs.push(format!("[Provision] 已分配: {gpu_label}"));
            session.logs.push(format!(
                "[Drive] DRIVE_MOUNT_OK True ({})",
                profile.driveRootPath
            ));
            session.logs.push(
                "[Watchdog] 云端独立看门狗已激活 (关闭桌面端依然自动防超时扣费)".to_string(),
            );
            session.logs.push(format!(
                "[Tunnel] 本地端口转发已就绪 -> http://127.0.0.1:{}",
                profile.comfyPort
            ));

            self.tunnels.write().await.push(PortTunnel {
                id: format!("tun-{}", profile.comfyPort),
                sessionName: session.name.clone(),
                remotePort: profile.comfyPort,
                localPort: profile.comfyPort,
                label: "ComfyUI 主服务".to_string(),
                status: TunnelStatus::Connected,
                healthy: Some(true),
                healthPath: Some("/system_stats".to_string()),
                reconnectCount: 0,
                lastError: None,
            });
            if let Some(mp) = profile.managementPort {
                self.tunnels.write().await.push(PortTunnel {
                    id: format!("tun-{mp}"),
                    sessionName: session.name.clone(),
                    remotePort: mp,
                    localPort: mp,
                    label: "DramaSmith 云端管理端".to_string(),
                    status: TunnelStatus::Connected,
                    healthy: Some(true),
                    healthPath: Some("/".to_string()),
                    reconnectCount: 0,
                    lastError: None,
                });
            }

            {
                let mut st = self.store.write().await;
                if let Some(s) = st.sessions.iter_mut().find(|x| x.name == session.name) {
                    *s = session.clone();
                }
            }
            self.persist().await;
            return Ok(session);
        }

        // REAL COLAB MODE PIPELINE
        let active_account = active_acc.ok_or_else(|| {
            "请先在「多账号与算力」或「设置」页面登录/接管一个 Google 账号（或右上角切到演示沙盒模式体验）".to_string()
        })?;
        let state_file = crate::colab::accounts::account_state_file(&self.data_dir, &active_account.id);

        // Stage 1: Provision Hardware (+ Min VRAM retry loop!)
        let prov = match crate::colab::session_runner::provision_with_vram_guard(
            &state_file,
            &session_name,
            profile.accelerator,
            profile.highMem,
            profile.minVramGb,
            profile.maxRetries,
        )
        .await
        {
            Ok(p) => p,
            Err(err) => {
                self.mark_stage_failed(&session_name, "provision", &err).await;
                return Err(err);
            }
        };

        self.update_session_mut(&session_name, |s| {
            s.endpoint = prov.endpoint.clone();
            s.gpuName = prov.gpu_name.clone();
            s.vramTotalMib = prov.vram_total_mib;
            s.vramUsedMib = prov.vram_used_mib;
            s.state = SessionLifecycle::Bootstrapping;
            set_stage(
                &mut s.stages,
                "provision",
                StageStatus::Completed,
                prov.gpu_name.clone(),
            );
            set_stage(&mut s.stages, "mount_drive", StageStatus::Running, None);
            s.logs.extend(prov.logs);
        })
        .await;

        // Deploy VM-side Watchdog immediately after VM allocation so cost guard holds even if setup fails!
        match crate::colab::session_runner::deploy_watchdog(
            &state_file,
            &session_name,
            profile.costGuard.autoStopMinutes,
            profile.costGuard.idleStopMinutes,
        )
        .await
        {
            Ok(()) => {
                self.update_session_mut(&session_name, |s| {
                    s.watchdogActive = true;
                    s.logs
                        .push("[Watchdog] 云端独立费用看门狗已部署并启动".to_string());
                })
                .await;
            }
            Err(w_err) => {
                self.update_session_mut(&session_name, |s| {
                    s.watchdogActive = false;
                    s.logs.push(format!(
                        "[Warning] 云端独立看门狗未能启动 ({w_err})，仅桌面端在线期间执行费用保护"
                    ));
                })
                .await;
            }
        }

        // Stage 2: Mount Google Drive
        if profile.driveMountMode != DriveMountMode::Disabled {
            let rclone_cfg = crate::colab::accounts::secret_get(&self.data_dir, "rclone_config")
                .await
                .ok()
                .flatten();
            match crate::colab::session_runner::mount_google_drive(
                &state_file,
                &session_name,
                profile.driveMountMode,
                rclone_cfg.as_deref(),
            )
            .await
            {
                Ok(mount_res) => {
                    self.update_session_mut(&session_name, |s| {
                        s.driveMounted = true;
                        s.driveAuthUrl = mount_res.auth_url;
                        set_stage(
                            &mut s.stages,
                            "mount_drive",
                            StageStatus::Completed,
                            Some(format!("DRIVE_MOUNT_OK ({})", profile.driveRootPath)),
                        );
                        set_stage(&mut s.stages, "restore_env", StageStatus::Running, None);
                        s.logs.push(format!(
                            "[Drive] Google Drive 已挂载 -> {}",
                            profile.driveRootPath
                        ));
                    })
                    .await;
                }
                Err(err) => {
                    self.mark_stage_failed(&session_name, "mount_drive", &err).await;
                    return Err(err);
                }
            }
        } else {
            self.update_session_mut(&session_name, |s| {
                set_stage(&mut s.stages, "mount_drive", StageStatus::Skipped, None);
                set_stage(&mut s.stages, "restore_env", StageStatus::Running, None);
            })
            .await;
        }

        // Stage 3: Execute ComfyUI / DramaSmith / Pre-Downloader bootstrap on remote VM
        let hf_token = crate::colab::accounts::secret_get(&self.data_dir, "hf_token")
            .await
            .ok()
            .flatten();
        let civ_token = crate::colab::accounts::secret_get(&self.data_dir, "civitai_token")
            .await
            .ok()
            .flatten();

        let startup_mode_str = match profile.startupMode {
            ProfileStartupMode::ManagedScript => "managedScript",
            ProfileStartupMode::ComfySnapshot => "comfySnapshot",
            ProfileStartupMode::CpuPreDownload => "cpuPreDownload",
        };
        let mut envs = vec![
            ("NBX_STARTUP_MODE".to_string(), startup_mode_str.to_string()),
            ("NBX_DRIVE_ROOT".to_string(), profile.driveRootPath.clone()),
            ("NBX_COMFY_PORT".to_string(), profile.comfyPort.to_string()),
            (
                "NBX_REQUIRED_NODES".to_string(),
                profile.requiredNodes.join(","),
            ),
        ];
        if let Some(rel) = &profile.managedScriptRelPath {
            envs.push(("NBX_MANAGED_SCRIPT".to_string(), rel.clone()));
        }
        if let Some(mp) = profile.managementPort {
            envs.push(("NBX_MGMT_PORT".to_string(), mp.to_string()));
        }
        if let Some(hf) = hf_token {
            envs.push(("HF_TOKEN".to_string(), hf));
        }
        if let Some(civ) = civ_token {
            envs.push(("CIVITAI_TOKEN".to_string(), civ));
        }

        let mut effective_mgmt_port = profile.managementPort;
        match crate::colab::session_runner::exec_python_on_session(
            &state_file,
            &session_name,
            crate::comfy::BOOTSTRAP_COMFY_PY,
            &envs,
            900,
        )
        .await
        {
            Ok(output) => {
                let mut cf_url = extract_trycloudflare_url(&output);
                for line in output.lines() {
                    if let Some(rest) = line.trim().strip_prefix("__NBX_STAGE__ ") {
                        if let Ok(ev) = serde_json::from_str::<serde_json::Value>(rest) {
                            if ev.get("stage").and_then(|v| v.as_str()) == Some("ready") {
                                if ev.get("managementPort").is_some_and(|v| v.is_null()) {
                                    effective_mgmt_port = None;
                                }
                                if let Some(u) = ev.get("cloudflareUrl").and_then(|v| v.as_str()) {
                                    cf_url = Some(u.to_string());
                                }
                            }
                        }
                    }
                }
                self.update_session_mut(&session_name, |s| {
                    s.cloudflareUrl = cf_url;
                    set_stage(
                        &mut s.stages,
                        "restore_env",
                        StageStatus::Completed,
                        Some("环境与模型目录映射完成".to_string()),
                    );
                    set_stage(&mut s.stages, "tunnel", StageStatus::Running, None);
                    for line in output.lines().take(30) {
                        if !line.trim().is_empty() {
                            s.logs.push(line.to_string());
                        }
                    }
                })
                .await;
            }
            Err(err) => {
                self.mark_stage_failed(&session_name, "restore_env", &err).await;
                return Err(err);
            }
        }

        // Stage 4: Open local 127.0.0.1 SSH port-forward tunnels
        if profile.startupMode != ProfileStartupMode::CpuPreDownload {
            let _ = self
                .create_port_tunnel(
                    &session_name,
                    profile.comfyPort,
                    profile.comfyPort,
                    "ComfyUI 主服务",
                    Some("/system_stats".to_string()),
                )
                .await?;
            if let Some(mp) = effective_mgmt_port {
                let _ = self
                    .create_port_tunnel(
                        &session_name,
                        mp,
                        mp,
                        "DramaSmith 管理端",
                        Some("/".to_string()),
                    )
                    .await;
            }
        }

        self.update_session_mut(&session_name, |s| {
            s.comfyLocalUrl = Some(format!("http://127.0.0.1:{}", profile.comfyPort));
            s.managementLocalUrl = effective_mgmt_port.map(|mp| format!("http://127.0.0.1:{mp}"));
            set_stage(
                &mut s.stages,
                "tunnel",
                StageStatus::Completed,
                Some(format!("127.0.0.1:{} 已连通", profile.comfyPort)),
            );
            set_stage(&mut s.stages, "readiness", StageStatus::Running, None);
        })
        .await;

        // Stage 5: Deep Readiness Check (/system_stats, /queue, required custom nodes e.g. H3C_Timeline)
        let mut loaded_nodes = Vec::new();
        let mut missing_nodes = Vec::new();
        if profile.startupMode != ProfileStartupMode::CpuPreDownload {
            let base = format!("http://127.0.0.1:{}", profile.comfyPort);
            let mut stats_ok = false;
            for _ in 0..30 {
                if let Ok(r) = self
                    .http_client
                    .get(format!("{base}/system_stats"))
                    .timeout(std::time::Duration::from_secs(4))
                    .send()
                    .await
                {
                    if r.status().is_success() {
                        stats_ok = true;
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }

            if stats_ok {
                for node in &profile.requiredNodes {
                    let ok = match self
                        .http_client
                        .get(format!("{base}/object_info/{node}"))
                        .timeout(std::time::Duration::from_secs(5))
                        .send()
                        .await
                    {
                        Ok(resp) => resp
                            .json::<serde_json::Value>()
                            .await
                            .map(|v| v.get(node).is_some())
                            .unwrap_or(false),
                        Err(_) => false,
                    };
                    if ok {
                        loaded_nodes.push(node.clone());
                    } else {
                        missing_nodes.push(node.clone());
                    }
                }
            }
        }

        if profile.syncDramaSmith && auto_sync_ds {
            let sync_target = self
                .gateway_bound_url
                .read()
                .await
                .clone()
                .unwrap_or_else(|| format!("http://127.0.0.1:{}", profile.comfyPort));
            let _ = crate::comfy::sync_dramasmith_colab_url(&self.http_client, &ds_url, &sync_target)
                .await;
        }

        let final_session = self
            .update_session_mut(&session_name, |s| {
                s.comfyReady = profile.startupMode != ProfileStartupMode::CpuPreDownload;
                s.loadedRequiredNodes = loaded_nodes.clone();
                s.missingRequiredNodes = missing_nodes.clone();
                s.state = SessionLifecycle::Ready;
                let detail = if !missing_nodes.is_empty() {
                    format!("ComfyUI 已通，但缺少节点: {}", missing_nodes.join(", "))
                } else if !loaded_nodes.is_empty() {
                    format!("出片自检通过 · 已加载 {}", loaded_nodes.join(", "))
                } else {
                    "出片级自检全部通过".to_string()
                };
                set_stage(&mut s.stages, "readiness", StageStatus::Completed, Some(detail));
            })
            .await
            .ok_or_else(|| "Session state lost".to_string())?;

        self.persist().await;
        Ok(final_session)
    }

    async fn mark_stage_failed(&self, session_name: &str, stage_id: &str, err: &str) {
        self.update_session_mut(session_name, |s| {
            s.state = SessionLifecycle::Error;
            s.lastError = Some(err.to_string());
            set_stage(
                &mut s.stages,
                stage_id,
                StageStatus::Failed,
                Some(err.to_string()),
            );
            s.logs.push(format!("[Error] {err}"));
        })
        .await;
        self.persist().await;
    }

    async fn update_session_mut<F>(&self, session_name: &str, f: F) -> Option<ColabSession>
    where
        F: FnOnce(&mut ColabSession),
    {
        let mut st = self.store.write().await;
        let sess = st.sessions.iter_mut().find(|s| s.name == session_name)?;
        f(sess);
        Some(sess.clone())
    }

    pub async fn stop_all_active_sessions(&self, force: bool) -> Result<(), String> {
        let names: Vec<String> = self
            .store
            .read()
            .await
            .sessions
            .iter()
            .filter(|s| s.state != SessionLifecycle::Stopped)
            .map(|s| s.name.clone())
            .collect();
        for name in names {
            let _ = self.stop_session(&name, force).await;
        }
        Ok(())
    }

    pub async fn stop_session(
        &self,
        session_name: &str,
        force: bool,
    ) -> Result<ColabSession, String> {
        let (mode, sess, safe_backup) = {
            let st = self.store.read().await;
            let found = st
                .sessions
                .iter()
                .find(|s| s.name == session_name)
                .cloned()
                .ok_or_else(|| format!("未找到会话 '{session_name}'"))?;
            (
                st.settings.backendMode,
                found,
                st.settings.defaultCostGuard.safeBackupBeforeStop,
            )
        };

        self.update_session_mut(session_name, |s| {
            s.state = SessionLifecycle::Stopping;
            s.logs
                .push("[Stop] 正在检查任务队列并执行云盘安全备份...".to_string());
        })
        .await;

        let mut verified = true;
        if mode == BackendMode::Real {
            let state_file =
                crate::colab::accounts::account_state_file(&self.data_dir, &sess.accountId);
            verified = crate::colab::session_runner::verified_safe_stop(
                &self.data_dir,
                &state_file,
                &sess.accountId,
                &sess.name,
                &sess.endpoint,
                sess.driveRootPath.as_deref(),
                sess.comfyLocalUrl.as_deref(),
                sess.managementLocalUrl.as_deref(),
                safe_backup && !force,
                force,
            )
            .await?;
        }

        // Stop tunnels attached to this session
        let tunnel_ids: Vec<String> = self
            .tunnels
            .read()
            .await
            .iter()
            .filter(|t| t.sessionName == session_name)
            .map(|t| t.id.clone())
            .collect();
        for tid in tunnel_ids {
            let _ = self.stop_port_tunnel(&tid).await;
        }

        let stopped = self
            .update_session_mut(session_name, |s| {
                s.state = SessionLifecycle::Stopped;
                s.comfyReady = false;
                s.watchdogActive = false;
                s.verifiedUnassigned = verified;
                s.logs.push(if verified {
                    "[Verified] Google Colab 后端已确认释放该实例，计费已 100% 结束。".to_string()
                } else {
                    "[Warning] 已发送释放指令，请刷新算力列表二次确认。".to_string()
                });
            })
            .await
            .ok_or_else(|| "Session disappeared".to_string())?;

        self.persist().await;
        Ok(stopped)
    }

    pub async fn extend_session_deadline(
        &self,
        session_name: &str,
        add_minutes: u32,
    ) -> Result<ColabSession, String> {
        self.touch_activity().await;
        let (mode, sess) = {
            let st = self.store.read().await;
            let found = st
                .sessions
                .iter()
                .find(|s| s.name == session_name)
                .cloned()
                .ok_or_else(|| format!("未找到会话 '{session_name}'"))?;
            (st.settings.backendMode, found)
        };

        let base_time = sess
            .autoStopAt
            .as_deref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let next_deadline = base_time + ChronoDuration::minutes(add_minutes as i64);

        if mode == BackendMode::Real {
            let state_file =
                crate::colab::accounts::account_state_file(&self.data_dir, &sess.accountId);
            let _ = crate::colab::session_runner::update_watchdog_deadline(
                &state_file,
                session_name,
                Some(next_deadline.timestamp() as f64),
                sess.idleMinutesLeft,
            )
            .await;
        }

        let updated = self
            .update_session_mut(session_name, |s| {
                s.autoStopAt = Some(next_deadline.to_rfc3339());
                s.logs.push(format!(
                    "[Watchdog] 已延长自动停机时间 +{add_minutes} 分钟"
                ));
            })
            .await
            .ok_or_else(|| "Session not found".to_string())?;
        self.persist().await;
        Ok(updated)
    }

    pub async fn save_comfy_environment_snapshot(
        &self,
        session_name: &str,
    ) -> Result<String, String> {
        let (mode, sess) = {
            let st = self.store.read().await;
            let found = st
                .sessions
                .iter()
                .find(|s| s.name == session_name)
                .cloned()
                .ok_or_else(|| format!("未找到运行中的会话 '{session_name}'"))?;
            (st.settings.backendMode, found)
        };
        let drive_root = sess
            .driveRootPath
            .clone()
            .unwrap_or_else(|| "/content/drive/MyDrive/DramaSmith-H3".to_string());

        if mode == BackendMode::Mock {
            self.update_session_mut(session_name, |s| {
                s.logs.push(format!(
                    "[Snapshot] 已将当前自定义节点(Custom Nodes)与 ComfyUI 配置打包保存至 {drive_root}/env-snapshot/comfyui_env.tar.gz"
                ));
            })
            .await;
            return Ok(format!(
                "已固化保存到 Google Drive: {drive_root}/env-snapshot/comfyui_env.tar.gz"
            ));
        }

        let state_file =
            crate::colab::accounts::account_state_file(&self.data_dir, &sess.accountId);
        let envs = vec![
            ("NBX_STARTUP_MODE".to_string(), "saveSnapshot".to_string()),
            ("NBX_DRIVE_ROOT".to_string(), drive_root.clone()),
        ];
        let out = crate::colab::session_runner::exec_python_on_session(
            &state_file,
            session_name,
            crate::comfy::BOOTSTRAP_COMFY_PY,
            &envs,
            600,
        )
        .await?;
        self.update_session_mut(session_name, |s| {
            s.logs
                .push(format!("[Snapshot] 环境快照保存完成: {}", out.trim()));
        })
        .await;
        Ok(format!(
            "配置与插件快照已永久保存至 {drive_root}/env-snapshot/"
        ))
    }

    pub async fn sync_dramasmith_endpoint(&self, endpoint_url: &str) -> Result<String, String> {
        let ds_url = self.store.read().await.settings.dramaSmithApiUrl.clone();
        crate::comfy::sync_dramasmith_colab_url(&self.http_client, &ds_url, endpoint_url).await
    }

    pub async fn create_port_tunnel(
        &self,
        session_name: &str,
        remote_port: u16,
        local_port: u16,
        label: &str,
        health_path: Option<String>,
    ) -> Result<PortTunnel, String> {
        let (mode, account_id) = {
            let st = self.store.read().await;
            let acc_id = st
                .sessions
                .iter()
                .find(|s| s.name == session_name)
                .map(|s| s.accountId.clone())
                .or_else(|| st.accounts.iter().find(|a| a.isActive).map(|a| a.id.clone()))
                .unwrap_or_else(|| "default".to_string());
            (st.settings.backendMode, acc_id)
        };

        let tunnel_id = format!("tun-{session_name}-{local_port}");
        let info = PortTunnel {
            id: tunnel_id.clone(),
            sessionName: session_name.to_string(),
            remotePort: remote_port,
            localPort: local_port,
            label: label.to_string(),
            status: if mode == BackendMode::Mock {
                TunnelStatus::Connected
            } else {
                TunnelStatus::Connecting
            },
            healthy: if mode == BackendMode::Mock {
                Some(true)
            } else {
                None
            },
            healthPath: health_path.clone(),
            reconnectCount: 0,
            lastError: None,
        };

        {
            let mut list = self.tunnels.write().await;
            list.retain(|t| t.id != tunnel_id);
            list.push(info.clone());
        }

        if mode == BackendMode::Real {
            let state_file = crate::colab::accounts::account_state_file(&self.data_dir, &account_id);
            let handle = crate::colab::tunnel::spawn_supervised_tunnel(
                state_file,
                session_name.to_string(),
                remote_port,
                local_port,
                health_path,
            )
            .await?;
            let live = handle.snapshot();
            self.tunnel_stops.lock().await.insert(tunnel_id.clone(), handle);
            let mut list = self.tunnels.write().await;
            if let Some(t) = list.iter_mut().find(|x| x.id == tunnel_id) {
                t.status = live.status;
                t.healthy = live.healthy;
                return Ok(t.clone());
            }
        }

        Ok(info)
    }

    pub async fn stop_port_tunnel(&self, tunnel_id: &str) -> Result<(), String> {
        if let Some(handle) = self.tunnel_stops.lock().await.remove(tunnel_id) {
            handle.stop().await;
        }
        self.tunnels.write().await.retain(|t| t.id != tunnel_id);
        Ok(())
    }

    pub async fn scan_workflow_models(
        &self,
        workflow_name: &str,
        workflow_json: &str,
    ) -> Result<WorkflowScanResult, String> {
        let drive_models = self.store.read().await.driveModels.clone();
        crate::comfy::scan_workflow_models(workflow_name, workflow_json, &drive_models)
    }

    pub async fn enqueue_model_downloads(
        &self,
        items: Vec<ModelDownloadInput>,
    ) -> Result<Vec<ModelDownloadTask>, String> {
        let mut st = self.store.write().await;
        for item in items {
            if st
                .downloads
                .iter()
                .any(|d| d.filename == item.filename && d.status != DownloadStatus::Failed)
            {
                continue;
            }
            let dir = item
                .targetDriveDir
                .unwrap_or_else(|| item.category.drive_subdir().to_string());
            st.downloads.push(ModelDownloadTask {
                id: format!("dl-{}", &Uuid::new_v4().to_string()[..8]),
                filename: item.filename,
                sourceUrl: item.sourceUrl,
                category: item.category,
                targetDriveDir: dir,
                status: DownloadStatus::Queued,
                progressPercent: 0,
                downloadedBytes: 0,
                totalBytes: None,
                error: None,
            });
        }
        let out = st.downloads.clone();
        drop(st);
        self.persist().await;
        Ok(out)
    }

    pub async fn remove_model_download(
        &self,
        task_id: &str,
    ) -> Result<Vec<ModelDownloadTask>, String> {
        let mut st = self.store.write().await;
        st.downloads.retain(|d| d.id != task_id);
        let out = st.downloads.clone();
        drop(st);
        self.persist().await;
        Ok(out)
    }

    pub async fn start_model_downloads(
        &self,
        use_cpu_mode: bool,
        session_name: Option<String>,
    ) -> Result<Vec<ModelDownloadTask>, String> {
        let (mode, drive_root, queued) = {
            let st = self.store.read().await;
            let q: Vec<ModelDownloadTask> = st
                .downloads
                .iter()
                .filter(|d| matches!(d.status, DownloadStatus::Queued | DownloadStatus::Failed))
                .cloned()
                .collect();
            (
                st.settings.backendMode,
                st.settings.defaultDriveRoot.clone(),
                q,
            )
        };

        if queued.is_empty() {
            return Ok(self.store.read().await.downloads.clone());
        }

        if mode == BackendMode::Mock {
            let mut st = self.store.write().await;
            let mut newly_added = Vec::new();
            for d in st.downloads.iter_mut() {
                if matches!(d.status, DownloadStatus::Queued | DownloadStatus::Failed) {
                    d.status = DownloadStatus::Completed;
                    d.progressPercent = 100;
                    d.downloadedBytes = 4_294_967_296;
                    d.totalBytes = Some(4_294_967_296);
                    newly_added.push(DriveModelItem {
                        filename: d.filename.clone(),
                        relativePath: format!("{}/{}", d.targetDriveDir, d.filename),
                        category: d.category,
                        sizeBytes: 4_294_967_296,
                        updatedAt: Some(Utc::now().to_rfc3339()),
                    });
                }
            }
            for item in newly_added {
                if !st.driveModels.iter().any(|m| m.filename == item.filename) {
                    st.driveModels.push(item);
                }
            }
            let out = st.downloads.clone();
            drop(st);
            self.persist().await;
            return Ok(out);
        }

        // Real Mode: either reuse active session or spin up temporary Zero-GPU CPU Pre-Downloader VM!
        let mut created_temp_cpu = false;
        let target_session = if let Some(existing_name) = session_name {
            self.store
                .read()
                .await
                .sessions
                .iter()
                .find(|s| s.name == existing_name && s.state != SessionLifecycle::Stopped)
                .cloned()
                .ok_or_else(|| format!("指定的会话 '{existing_name}' 未运行"))?
        } else if !use_cpu_mode {
            self.store
                .read()
                .await
                .sessions
                .iter()
                .find(|s| s.driveMounted && s.state != SessionLifecycle::Stopped)
                .cloned()
                .ok_or_else(|| "当前没有已挂载云盘的运行实例，请勾选「CPU 省积分备货模式」自动拉起下载".to_string())?
        } else {
            created_temp_cpu = true;
            self.launch_profile_session("cpu-model-preloader").await?
        };

        let state_file = crate::colab::accounts::account_state_file(
            &self.data_dir,
            &target_session.accountId,
        );
        let queue_json = serde_json::to_string(&queued).map_err(|e| e.to_string())?;
        let hf_token = crate::colab::accounts::secret_get(&self.data_dir, "hf_token")
            .await
            .ok()
            .flatten();
        let civ_token = crate::colab::accounts::secret_get(&self.data_dir, "civitai_token")
            .await
            .ok()
            .flatten();

        let mut envs = vec![
            ("NBX_STARTUP_MODE".to_string(), "cpuPreDownload".to_string()),
            ("NBX_DRIVE_ROOT".to_string(), drive_root),
            ("NBX_DOWNLOAD_QUEUE_JSON".to_string(), queue_json),
        ];
        if let Some(hf) = hf_token {
            envs.push(("HF_TOKEN".to_string(), hf));
        }
        if let Some(civ) = civ_token {
            envs.push(("CIVITAI_TOKEN".to_string(), civ));
        }

        let run_res = crate::colab::session_runner::exec_python_on_session(
            &state_file,
            &target_session.name,
            crate::comfy::BOOTSTRAP_COMFY_PY,
            &envs,
            3600,
        )
        .await;

        if created_temp_cpu {
            // Auto-shutdown CPU VM right after downloads finish so zero resources linger!
            let _ = self.stop_session(&target_session.name, true).await;
        }

        match run_res {
            Ok(_) => {
                let mut st = self.store.write().await;
                let mut added = Vec::new();
                for d in st.downloads.iter_mut() {
                    if matches!(d.status, DownloadStatus::Queued | DownloadStatus::Downloading) {
                        d.status = DownloadStatus::Completed;
                        d.progressPercent = 100;
                        added.push(DriveModelItem {
                            filename: d.filename.clone(),
                            relativePath: format!("{}/{}", d.targetDriveDir, d.filename),
                            category: d.category,
                            sizeBytes: d.totalBytes.unwrap_or(0),
                            updatedAt: Some(Utc::now().to_rfc3339()),
                        });
                    }
                }
                for m in added {
                    if !st.driveModels.iter().any(|x| x.filename == m.filename) {
                        st.driveModels.push(m);
                    }
                }
                let out = st.downloads.clone();
                drop(st);
                self.persist().await;
                Ok(out)
            }
            Err(err) => {
                let mut st = self.store.write().await;
                for d in st.downloads.iter_mut() {
                    if d.status == DownloadStatus::Queued {
                        d.status = DownloadStatus::Failed;
                        d.error = Some(err.clone());
                    }
                }
                drop(st);
                self.persist().await;
                Err(err)
            }
        }
    }

    pub async fn save_api_workflow(
        &self,
        mut workflow: SavedApiWorkflow,
    ) -> Result<Vec<SavedApiWorkflow>, String> {
        workflow.updatedAt = Utc::now().to_rfc3339();
        let mut st = self.store.write().await;
        if let Some(existing) = st.apiWorkflows.iter_mut().find(|w| w.id == workflow.id) {
            *existing = workflow;
        } else {
            st.apiWorkflows.push(workflow);
        }
        let out = st.apiWorkflows.clone();
        drop(st);
        self.persist().await;
        Ok(out)
    }

    pub async fn delete_api_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<SavedApiWorkflow>, String> {
        let mut st = self.store.write().await;
        st.apiWorkflows.retain(|w| w.id != workflow_id);
        let out = st.apiWorkflows.clone();
        drop(st);
        self.persist().await;
        Ok(out)
    }

    pub async fn run_api_workflow(
        &self,
        req: WorkflowRunRequest,
    ) -> Result<WorkflowRunResult, String> {
        let started_instant = std::time::Instant::now();
        self.touch_activity().await;

        let (mode, saved_wf, parent_graph) = {
            let st = self.store.read().await;
            let wf = req
                .workflowId
                .as_ref()
                .and_then(|id| st.apiWorkflows.iter().find(|w| &w.id == id).cloned());
            let parent = req
                .parentTaskId
                .as_ref()
                .filter(|s| !s.trim().is_empty())
                .and_then(|pid| st.completedGraphs.get(pid.trim()).cloned());
            (st.settings.backendMode, wf, parent)
        };

        // Build & validate ComfyUI API prompt graph BEFORE cold-starting a paid GPU!
        let is_continue = req.continuityMode.as_deref() == Some("continue");
        if is_continue && parent_graph.is_none() {
            return Err("续接模式 (continue) 需要有效的已完成 parentTaskId 以读取上一段 H3 时间线".to_string());
        }

        let job_id = format!("job-{}", &Uuid::new_v4().to_string()[..8]);
        let mut effective_seq_name = req.sequenceName.clone();

        let mut graph = if let Some(raw) = req.rawPromptGraph {
            raw
        } else if let Some(wf) = &saved_wf {
            let mut g = crate::comfy::apply_workflow_params(wf, &req.params)?;
            if wf.supportsContinuity {
                let seq = effective_seq_name
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| {
                        parent_graph
                            .as_ref()
                            .and_then(|p| p.pointer("/100/inputs/name"))
                            .and_then(|v| v.as_str())
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "wenwu_bf16_continuity".to_string());
                crate::comfy::configure_h3_sequence(
                    &mut g,
                    &seq,
                    if is_continue { parent_graph.as_ref() } else { None },
                )?;
                effective_seq_name = Some(seq);
            }
            g
        } else {
            return Err("请指定 workflowId 或直接传入 rawPromptGraph".to_string());
        };

        if let Some(obj) = graph.as_object_mut() {
            if obj.is_empty() {
                return Err("工作流 API 节点图为空".to_string());
            }
        }

        let target_profile_id = req
            .profileId
            .clone()
            .or_else(|| saved_wf.as_ref().map(|w| w.profileId.clone()))
            .unwrap_or_else(|| "dramasmith-h3-bf16".to_string());

        let already_ready = {
            let st = self.store.read().await;
            st.sessions
                .iter()
                .find(|s| s.comfyReady && matches!(s.state, SessionLifecycle::Ready | SessionLifecycle::Busy))
                .cloned()
        };

        let (session, cold_started) = match already_ready {
            Some(s) => (s, false),
            None => {
                if !req.autoWake {
                    return Err("云端 ComfyUI 当前未运行，且请求禁用了 autoWake 自动开机".to_string());
                }
                let booted = self.launch_profile_session(&target_profile_id).await?;
                (booted, true)
            }
        };

        let seq_path = effective_seq_name.as_ref().map(|seq| {
            format!(
                "{}/outputs/h3_sequences/{seq}",
                session
                    .driveRootPath
                    .as_deref()
                    .unwrap_or("/content/drive/MyDrive/DramaSmith-H3")
            )
        });

        if mode == BackendMode::Mock {
            let mock_prompt_id = format!("prompt-{}", &Uuid::new_v4().to_string()[..8]);
            {
                let mut st = self.store.write().await;
                st.completedGraphs.insert(job_id.clone(), graph.clone());
                st.completedGraphs.insert(mock_prompt_id.clone(), graph);
            }
            self.persist().await;
            let ext = if saved_wf
                .as_ref()
                .map(|w| w.outputKind.as_str())
                .unwrap_or("video")
                == "image"
            {
                "png"
            } else {
                "mp4"
            };
            return Ok(WorkflowRunResult {
                jobId: job_id.clone(),
                promptId: Some(mock_prompt_id),
                sessionName: session.name,
                coldStarted: cold_started,
                status: "completed".to_string(),
                outputs: vec![format!(
                    "http://127.0.0.1:18188/view?filename={job_id}_output.{ext}&type=output"
                )],
                driveSequencePath: seq_path,
                elapsedSeconds: started_instant.elapsed().as_secs_f64(),
                error: None,
            });
        }

        // Submit to real ComfyUI via local SSH tunnel
        let comfy_base = session
            .comfyLocalUrl
            .unwrap_or_else(|| "http://127.0.0.1:8188".to_string());
        let submit_resp = self
            .http_client
            .post(format!("{comfy_base}/prompt"))
            .json(&json!({
                "prompt": graph,
                "client_id": job_id,
            }))
            .send()
            .await
            .map_err(|e| format!("提交工作流到 ComfyUI 失败: {e}"))?;

        let submit_json: serde_json::Value = submit_resp
            .json()
            .await
            .map_err(|e| format!("解析 ComfyUI /prompt 响应失败: {e}"))?;

        let prompt_id = submit_json
            .get("prompt_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("ComfyUI 拒绝执行该工作流: {submit_json}"))?
            .to_string();

        if !req.waitForCompletion {
            return Ok(WorkflowRunResult {
                jobId: job_id,
                promptId: Some(prompt_id),
                sessionName: session.name,
                coldStarted: cold_started,
                status: "queued".to_string(),
                outputs: Vec::new(),
                driveSequencePath: seq_path,
                elapsedSeconds: started_instant.elapsed().as_secs_f64(),
                error: None,
            });
        }

        // Poll /history/{prompt_id} until completed
        let gw_base = self
            .gateway_bound_url
            .read()
            .await
            .clone()
            .unwrap_or_else(|| comfy_base.clone());

        for _ in 0..900 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            self.touch_activity().await;

            if let Ok(h_resp) = self
                .http_client
                .get(format!("{comfy_base}/history/{prompt_id}"))
                .send()
                .await
            {
                if let Ok(h_json) = h_resp.json::<serde_json::Value>().await {
                    if let Some(entry) = h_json.get(&prompt_id) {
                        let mut urls = Vec::new();
                        if let Some(outputs_obj) =
                            entry.get("outputs").and_then(|o| o.as_object())
                        {
                            for (_node_id, node_out) in outputs_obj {
                                for key in ["gifs", "videos", "images", "audio"] {
                                    if let Some(arr) =
                                        node_out.get(key).and_then(|a| a.as_array())
                                    {
                                        for item in arr {
                                            if let Some(fname) =
                                                item.get("filename").and_then(|f| f.as_str())
                                            {
                                                let sub = item
                                                    .get("subfolder")
                                                    .and_then(|s| s.as_str())
                                                    .unwrap_or("");
                                                let ftype = item
                                                    .get("type")
                                                    .and_then(|t| t.as_str())
                                                    .unwrap_or("output");
                                                urls.push(format!(
                                                    "{gw_base}/view?filename={fname}&subfolder={sub}&type={ftype}"
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        {
                            let mut st = self.store.write().await;
                            st.completedGraphs.insert(job_id.clone(), graph.clone());
                            st.completedGraphs.insert(prompt_id.clone(), graph);
                        }
                        self.persist().await;
                        return Ok(WorkflowRunResult {
                            jobId: job_id,
                            promptId: Some(prompt_id),
                            sessionName: session.name,
                            coldStarted: cold_started,
                            status: "completed".to_string(),
                            outputs: urls,
                            driveSequencePath: seq_path,
                            elapsedSeconds: started_instant.elapsed().as_secs_f64(),
                            error: None,
                        });
                    }
                }
            }
        }

        Err("等待 ComfyUI 工作流生成超时（任务仍在云端队列运行，可在 ComfyUI 窗口查看）".to_string())
    }

    pub async fn inspect_gguf_repo(
        &self,
        repo_id: &str,
        target_vram_mib: u32,
    ) -> Result<GgufRepoInspection, String> {
        let hf_token = crate::colab::accounts::secret_get(&self.data_dir, "hf_token")
            .await
            .ok()
            .flatten();
        crate::comfy::inspect_hf_gguf_repo(
            &self.http_client,
            repo_id,
            target_vram_mib,
            hf_token.as_deref(),
        )
        .await
    }

    pub async fn deploy_gguf_model(
        &self,
        repo_id: &str,
        accelerator: AcceleratorKind,
        min_vram_gb: u32,
        quant: Option<String>,
        local_port: u16,
    ) -> Result<LlamaDeployment, String> {
        self.touch_activity().await;
        let mode = self.store.read().await.settings.backendMode;
        let target_vram = if min_vram_gb > 0 {
            min_vram_gb * 1024
        } else {
            accelerator.nominal_vram_mib().max(15_360)
        };

        let chosen_quant = match quant.clone() {
            Some(q) if !q.trim().is_empty() && q != "auto" => q,
            _ => self
                .inspect_gguf_repo(repo_id, target_vram)
                .await
                .ok()
                .and_then(|ins| ins.autoPickedQuant)
                .unwrap_or_else(|| "Q4_K_M".to_string()),
        };

        if mode == BackendMode::Mock {
            let dep = LlamaDeployment {
                id: format!("llama-{}", &Uuid::new_v4().to_string()[..6]),
                sessionName: format!("nbx-llama-{}", &Uuid::new_v4().to_string()[..5]),
                repoId: repo_id.to_string(),
                chosenQuant: chosen_quant.clone(),
                ctxSize: if target_vram >= 40_000 { 32768 } else { 16384 },
                localPort: local_port,
                apiBaseUrl: format!("http://127.0.0.1:{local_port}/v1"),
                webChatUrl: format!("http://127.0.0.1:{local_port}"),
                status: "ready".to_string(),
                logs: vec![
                    format!("[llama.cpp] 已根据 {target_vram} MiB 显存自动选定最优量化: {chosen_quant}"),
                    format!("[llama.cpp] OpenAI 兼容服务就绪: http://127.0.0.1:{local_port}/v1"),
                ],
            };
            let mut st = self.store.write().await;
            st.llamaDeployments.insert(0, dep.clone());
            drop(st);
            self.persist().await;
            return Ok(dep);
        }

        let active_acc = self
            .store
            .read()
            .await
            .accounts
            .iter()
            .find(|a| a.isActive)
            .cloned()
            .ok_or_else(|| "请先登录 Google 账号后再部署 GGUF 模型".to_string())?;

        let session_name = format!("nbx-llama-{}", &Uuid::new_v4().to_string()[..6]);
        let state_file =
            crate::colab::accounts::account_state_file(&self.data_dir, &active_acc.id);

        let prov = crate::colab::session_runner::provision_with_vram_guard(
            &state_file,
            &session_name,
            accelerator,
            false,
            min_vram_gb,
            3,
        )
        .await?;

        let _ = crate::colab::session_runner::deploy_watchdog(
            &state_file,
            &session_name,
            Some(180),
            Some(30),
        )
        .await;

        let requested_quant = quant
            .clone()
            .filter(|q| !q.trim().is_empty() && q != "auto")
            .unwrap_or_else(|| "auto".to_string());
        let hf_token = crate::colab::accounts::secret_get(&self.data_dir, "hf_token")
            .await
            .ok()
            .flatten();
        let mut envs = vec![
            ("PLUGIN_INPUT_REPO".to_string(), repo_id.to_string()),
            ("PLUGIN_INPUT_QUANT".to_string(), requested_quant),
            ("PLUGIN_INPUT_PORT".to_string(), local_port.to_string()),
        ];
        if let Some(hf) = hf_token {
            envs.push(("PLUGIN_HF_TOKEN".to_string(), hf));
        }

        let worker_out = crate::colab::session_runner::exec_python_on_session(
            &state_file,
            &session_name,
            crate::comfy::LLAMA_WORKER_PY,
            &envs,
            1200,
        )
        .await?;

        let mut actual_quant = chosen_quant;
        let mut actual_ctx = if prov.vram_total_mib.unwrap_or(16000) >= 40_000 {
            32768
        } else {
            16384
        };
        let mut actual_port = local_port;
        for line in worker_out.lines() {
            if let Some(rest) = line.trim().strip_prefix("__NBX_LLAMA__ ") {
                if let Ok(meta) = serde_json::from_str::<serde_json::Value>(rest) {
                    if let Some(q) = meta.get("chosenQuant").and_then(|v| v.as_str()) {
                        actual_quant = q.to_string();
                    }
                    if let Some(c) = meta.get("ctxSize").and_then(|v| v.as_u64()) {
                        actual_ctx = c as u32;
                    }
                    if let Some(p) = meta.get("port").and_then(|v| v.as_u64()) {
                        actual_port = p as u16;
                    }
                }
            }
        }

        let _ = self
            .create_port_tunnel(
                &session_name,
                actual_port,
                local_port,
                "llama.cpp OpenAI API",
                Some("/health".to_string()),
            )
            .await?;

        let dep = LlamaDeployment {
            id: format!("llama-{}", &Uuid::new_v4().to_string()[..6]),
            sessionName: session_name.clone(),
            repoId: repo_id.to_string(),
            chosenQuant: actual_quant,
            ctxSize: actual_ctx,
            localPort: local_port,
            apiBaseUrl: format!("http://127.0.0.1:{local_port}/v1"),
            webChatUrl: format!("http://127.0.0.1:{local_port}"),
            status: "ready".to_string(),
            logs: worker_out.lines().map(|s| s.to_string()).collect(),
        };

        let mut st = self.store.write().await;
        st.sessions.insert(
            0,
            ColabSession {
                id: Uuid::new_v4().to_string(),
                name: session_name,
                accountId: active_acc.id,
                accountEmail: active_acc.email,
                endpoint: prov.endpoint,
                accelerator,
                gpuName: prov.gpu_name,
                vramTotalMib: prov.vram_total_mib,
                vramUsedMib: prov.vram_used_mib,
                highMem: false,
                profileId: None,
                profileName: Some(format!("llama.cpp · {repo_id}")),
                state: SessionLifecycle::Ready,
                stages: Vec::new(),
                driveMounted: false,
                driveRootPath: None,
                driveAuthUrl: None,
                comfyLocalUrl: None,
                managementLocalUrl: None,
                cloudflareUrl: None,
                comfyReady: false,
                loadedRequiredNodes: Vec::new(),
                missingRequiredNodes: Vec::new(),
                queueRunning: 0,
                queuePending: 0,
                watchdogActive: true,
                autoStopAt: Some((Utc::now() + ChronoDuration::minutes(180)).to_rfc3339()),
                idleMinutesLeft: Some(30),
                spentCcu: 0.0,
                startedAt: Utc::now().to_rfc3339(),
                verifiedUnassigned: false,
                lastError: None,
                logs: dep.logs.clone(),
            },
        );
        st.llamaDeployments.insert(0, dep.clone());
        drop(st);
        self.persist().await;
        Ok(dep)
    }

    pub async fn chat_completion_proxy(
        &self,
        api_base_url: &str,
        messages: Vec<serde_json::Value>,
        model: Option<String>,
    ) -> Result<String, String> {
        let mode = self.store.read().await.settings.backendMode;
        if mode == BackendMode::Mock {
            let last_user = messages
                .iter()
                .rev()
                .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
                .and_then(|m| m.get("content").and_then(|c| c.as_str()))
                .unwrap_or("");
            return Ok(format!(
                "【演示沙盒回复】llama.cpp OpenAI 兼容端点 ({api_base_url}) 已收到消息：「{last_user}」。切换到实盘 Colab 模式部署后将直接由云端 GPU 推理返回！"
            ));
        }

        let resp = self
            .http_client
            .post(format!("{}/chat/completions", api_base_url.trim_end_matches('/')))
            .json(&json!({
                "model": model.unwrap_or_else(|| "local-gguf".to_string()),
                "messages": messages,
                "stream": false,
            }))
            .send()
            .await
            .map_err(|e| format!("请求本地 llama.cpp API 失败: {e}"))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("解析推理响应失败: {e}"))?;
        let text = body
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("异常响应结构: {body}"))?;
        Ok(text.to_string())
    }

    pub async fn active_comfy_local_port(&self) -> Option<u16> {
        self.tunnels
            .read()
            .await
            .iter()
            .find(|t| t.remotePort == 8188 || t.label.contains("ComfyUI"))
            .map(|t| t.localPort)
    }

    pub async fn active_management_local_port(&self) -> Option<u16> {
        self.tunnels
            .read()
            .await
            .iter()
            .find(|t| t.remotePort == 8189 || t.label.contains("管理"))
            .map(|t| t.localPort)
    }

    pub async fn auto_wake_check_info(&self) -> (bool, bool, String) {
        let st = self.store.read().await;
        let has_ready = st
            .sessions
            .iter()
            .any(|s| s.comfyReady && matches!(s.state, SessionLifecycle::Ready | SessionLifecycle::Busy));
        let default_prof = st
            .profiles
            .first()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| "dramasmith-h3-bf16".to_string());
        (st.settings.autoWakeOnApiCall, has_ready, default_prof)
    }
}

impl WorkflowProfile {
    fn accelerator_slug(&self) -> &'static str {
        match self.accelerator {
            AcceleratorKind::Cpu => "cpu",
            AcceleratorKind::T4 => "t4",
            AcceleratorKind::L4 => "l4",
            AcceleratorKind::G4 => "g4",
            AcceleratorKind::A100 => "a100",
            AcceleratorKind::H100 => "h100",
            AcceleratorKind::V5e1 => "v5e1",
            AcceleratorKind::V6e1 => "v6e1",
        }
    }
}

fn set_stage(stages: &mut [StartupStage], id: &str, status: StageStatus, detail: Option<String>) {
    if let Some(st) = stages.iter_mut().find(|x| x.id == id) {
        st.status = status;
        if detail.is_some() {
            st.detail = detail;
        }
    }
}

fn extract_trycloudflare_url(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com").ok()?;
    re.find(text).map(|m| m.as_str().to_string())
}
