use super::accounts::{account_helper, account_id, run, scoped_cli, session_endpoint};
use crate::types::{AcceleratorKind, DriveMountMode};
use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{Mutex as AsyncMutex, OwnedMutexGuard},
    time::{sleep, timeout},
};
use uuid::Uuid;

pub const ASSETS_WATCHDOG_PY: &str = include_str!("../../assets/watchdog.py");

#[derive(Debug, Clone)]
pub struct ProvisionedSession {
    pub endpoint: String,
    pub gpu_name: Option<String>,
    pub vram_total_mib: Option<u32>,
    pub vram_used_mib: Option<u32>,
    pub logs: Vec<String>,
}

async fn session_lock(state: &Path, name: &str) -> OwnedMutexGuard<()> {
    type Locks = HashMap<(PathBuf, String), Weak<AsyncMutex<()>>>;
    static LOCKS: OnceLock<Mutex<Locks>> = OnceLock::new();
    let lock = {
        let mut locks = LOCKS.get_or_init(Mutex::default).lock().unwrap();
        locks.retain(|_, v| v.strong_count() != 0);
        let weak = locks.entry((state.into(), name.into())).or_default();
        match weak.upgrade() {
            Some(lock) => lock,
            None => {
                let lock = Arc::new(AsyncMutex::new(()));
                *weak = Arc::downgrade(&lock);
                lock
            }
        }
    };
    lock.lock_owned().await
}

fn new_args(name: &str, accelerator: AcceleratorKind, high_mem: bool) -> Vec<&str> {
    let mut args = vec!["new", "-s", name];
    if let Some((flag, value)) = accelerator.cli_flag_pair() {
        args.extend([flag, value]);
    }
    if high_mem {
        args.push("--high-mem");
    }
    args
}

fn sufficient_vram(mib: u32, minimum_gb: u32) -> bool {
    u64::from(mib) >= (u64::from(minimum_gb) * 1024).saturating_sub(1024)
}

async fn cli(
    state: &Path,
    args: &[&str],
    stdin: Option<&str>,
    seconds: u64,
) -> Result<String, String> {
    let mut cmd = scoped_cli(state).await?;
    cmd.args(args);
    let out = run(cmd, stdin, seconds).await?;
    if !out.status.success() {
        return Err(format!("Colab {} exited with {}.", args[0], out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

async fn exec_snippet(state: &Path, name: &str, code: &str) -> Result<String, String> {
    exec_python_on_session(state, name, code, &[], 900).await
}

pub async fn exec_python_on_session(
    state: &Path,
    name: &str,
    code: &str,
    env: &[(String, String)],
    seconds: u64,
) -> Result<String, String> {
    let mark = format!("__NBX_DONE_{}", Uuid::new_v4().simple());
    let mut prelude = String::new();
    if !env.is_empty() {
        // --env is saved verbatim in the CLI's execution history. Transfer secret
        // values separately and unlink them before starting the user's script.
        let remote = format!("root/.nbx-env-{}", Uuid::new_v4().simple());
        let payload: std::collections::BTreeMap<_, _> = env.iter().map(|(k, v)| (k, v)).collect();
        let payload = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        cli(
            state,
            &["_upload-env", "-s", name, &remote],
            Some(&payload),
            90,
        )
        .await?;
        prelude = format!("import os, json\nfrom pathlib import Path\n_nbx_env=Path('/{remote}')\ntry:\n    os.environ.update(json.loads(_nbx_env.read_text()))\nfinally:\n    _nbx_env.unlink(missing_ok=True)\n");
        // Share only these non-secret settings with the detached watchdog.
        if env.iter().any(|(key, _)| key == "NBX_COMFY_PORT") {
            prelude.push_str(r#"
import tempfile
_nbx_root = Path('/root/.notebookx')
_nbx_root.mkdir(mode=0o700, exist_ok=True)
with tempfile.NamedTemporaryFile(mode='w', dir=_nbx_root, delete=False) as _f:
    json.dump({k: os.environ[k] for k in ('NBX_COMFY_PORT','NBX_MGMT_PORT','NBX_DRIVE_ROOT') if k in os.environ}, _f)
os.replace(_f.name, _nbx_root/'runtime-settings.json')
"#);
        }
    }
    // CLI exec may exit zero after the kernel emits a Python exception.
    let input = format!(
        r#"{prelude}
from contextlib import contextmanager as _nbx_context
@_nbx_context
def _nbx_activity():
    from pathlib import Path
    from threading import Event, Thread
    beat = Path('/root/.notebookx/heartbeat')
    beat.parent.mkdir(mode=0o700, exist_ok=True)
    beat.touch()
    stopped = Event()
    def update():
        while not stopped.wait(10):
            beat.touch()
    worker = Thread(target=update, daemon=True)
    worker.start()
    try:
        yield
    finally:
        stopped.set()
        worker.join()
with _nbx_activity():
    exec(compile({}, '<notebookx>', 'exec'), globals())
print('\n{mark}', flush=True)
"#,
        literal(code)
    );
    let output = cli(
        state,
        &["exec", "-s", name, "--timeout", &seconds.to_string()],
        Some(&input),
        seconds.saturating_add(60),
    )
    .await?;
    let (before, _) = output
        .rsplit_once(&mark)
        .ok_or("Remote Python did not finish successfully.")?;
    Ok(before.trim_end().to_owned())
}

fn parse_gpu(text: &str) -> Result<(String, u32, u32), String> {
    for line in text.lines() {
        let parts: Vec<_> = line.rsplitn(3, ',').map(str::trim).collect();
        if let [used, total, name] = parts.as_slice() {
            if let (Ok(total), Ok(used)) = (total.parse::<u32>(), used.parse::<u32>()) {
                if !name.is_empty() && total > 0 && used <= total {
                    return Ok((name.to_string(), total, used));
                }
            }
        }
    }
    Err("Could not read GPU memory from nvidia-smi.".into())
}

async fn assigned(state: &Path, endpoint: &str) -> Result<bool, String> {
    let value = account_helper(&["assignments", account_id(state)?], None).await?;
    let rows = value
        .as_array()
        .ok_or("Invalid Google assignments response.")?;
    for row in rows {
        if row["endpoint"]
            .as_str()
            .ok_or("Invalid assignment record.")?
            == endpoint
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn release(state: &Path, name: &str, endpoint: &str) -> Result<(), String> {
    let account = account_id(state)?;
    // Session names can be reused. Never stop the new owner of a name.
    if matches!(session_endpoint(state, name).await, Ok(Some(record)) if record == endpoint) {
        let _ = cli(state, &["stop", "-s", name], None, 90).await;
    }
    // An upstream stop may already have released it. Only the assignment list
    // can distinguish that from a failed stop or a missing local state record.
    let _ = account_helper(&["unassign", endpoint, account], None).await;
    let mut last = String::new();
    for delay in [0, 1, 2, 4, 8] {
        if delay > 0 {
            sleep(Duration::from_secs(delay)).await;
        }
        match assigned(state, endpoint).await {
            Ok(false) => {
                forget_released(state, name, endpoint).await?;
                return Ok(());
            }
            Ok(true) => last = "Google still reports the assignment".into(),
            Err(e) => last = e,
        }
    }
    Err(format!("Release of {endpoint} is unverified. {last}."))
}

async fn forget_released(state: &Path, name: &str, endpoint: &str) -> Result<(), String> {
    if session_endpoint(state, name).await?.as_deref() == Some(endpoint) {
        cli(state, &["_forget-released", name, endpoint], None, 20).await
            .map_err(|_| format!("Google released {endpoint}, but its local session record could not be removed."))?;
    }
    Ok(())
}

pub async fn provision_with_vram_guard(
    state: &Path,
    name: &str,
    accelerator: AcceleratorKind,
    high_mem: bool,
    min_vram_gb: u32,
    max_retries: u32,
) -> Result<ProvisionedSession, String> {
    let _guard = session_lock(state, name).await;
    let args = new_args(name, accelerator, high_mem);
    let mut logs = Vec::new();
    for attempt in 0..=max_retries {
        if session_endpoint(state, name).await?.is_some() {
            return Err("Session name already exists.".into());
        }
        let created = cli(state, &args, None, 300).await;
        let endpoint = session_endpoint(state, name).await?.ok_or_else(|| {
            format!(
                "{} No endpoint was recorded. Check this account's assignments before retrying.",
                created
                    .as_ref()
                    .err()
                    .map(String::as_str)
                    .unwrap_or("Allocation status unknown.")
            )
        })?;
        if let Err(e) = created {
            release(state, name, &endpoint).await?;
            return Err(e);
        }
        let mut vm = ProvisionedSession {
            endpoint,
            gpu_name: None,
            vram_total_mib: None,
            vram_used_mib: None,
            logs: std::mem::take(&mut logs),
        };
        if !accelerator.is_gpu() {
            return Ok(vm);
        }
        let probe = "import subprocess\nprint(subprocess.check_output(['nvidia-smi','--query-gpu=name,memory.total,memory.used','--format=csv,noheader,nounits'],text=True))";
        let result = exec_snippet(state, name, probe)
            .await
            .and_then(|s| parse_gpu(&s));
        let reason = match result {
            Ok((gpu, total, used)) if sufficient_vram(total, min_vram_gb) => {
                vm.gpu_name = Some(gpu);
                vm.vram_total_mib = Some(total);
                vm.vram_used_mib = Some(used);
                return Ok(vm);
            }
            Ok((gpu, total, _)) => format!(
                "{gpu} has {total} MiB. This profile requests {} GiB.",
                min_vram_gb
            ),
            Err(e) => e,
        };
        release(state, name, &vm.endpoint).await?;
        logs = vm.logs;
        logs.push(format!("{reason} Released {}.", vm.endpoint));
        if attempt == max_retries {
            return Err(format!("{reason} Rejected assignment released."));
        }
    }
    unreachable!()
}

async fn native_drive<F, Fut>(state: &Path, name: &str, mut consent: F) -> Result<(), String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let mut cmd = scoped_cli(state).await?;
    let mut child = cmd
        .args(["drivemount", "-s", name, "/content/drive"])
        .env("PYTHONUNBUFFERED", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut stdin = child.stdin.take().unwrap();
    timeout(Duration::from_secs(630), async {
        let (mut out_open, mut err_open) = (true, true);
        while out_open || err_open {
            let line = tokio::select! {
                line = stdout.next_line(), if out_open => { let v = line.map_err(|e| e.to_string())?; out_open = v.is_some(); v },
                line = stderr.next_line(), if err_open => { let v = line.map_err(|e| e.to_string())?; err_open = v.is_some(); v },
            };
            if let Some(url) = line.as_deref().map(str::trim).filter(|s| s.starts_with("https://")) {
                let url = reqwest::Url::parse(url).map_err(|_| "Malformed Drive consent URL.")?;
                let host = url.host_str().unwrap_or("");
                if host == "google.com" || host.ends_with(".google.com") {
                    consent(url.to_string()).await?;
                    stdin.write_all(b"\n").await.map_err(|e| e.to_string())?;
                }
            }
        }
        if child.wait().await.map_err(|e| e.to_string())?.success() { Ok(()) } else { Err("Colab could not mount Drive.".into()) }
    }).await.map_err(|_| "Timed out waiting for Drive authorization.")?
}

const RCLONE_MOUNT: &str = r#"
import configparser, os, pathlib, shutil, subprocess
conf = pathlib.Path('/root/.notebookx/rclone.conf')
os.chmod(conf, 0o600)
if not shutil.which('rclone'):
    subprocess.run(['apt-get', 'update', '-qq'], check=True)
    subprocess.run(['apt-get', 'install', '-y', 'rclone', 'fuse3'], check=True)
drive = pathlib.Path('/content/drive/MyDrive')
drive.mkdir(parents=True, exist_ok=True)
if not os.path.ismount(drive):
    config = configparser.ConfigParser(interpolation=None)
    config.read(conf)
    remote = next(s for s in config.sections() if config.get(s, 'type', fallback='') == 'drive')
    subprocess.run(['rclone', '--config', str(conf), 'mount', remote + ':', str(drive),
                    '--daemon', '--vfs-cache-mode', 'writes', '--vfs-write-back', '0s',
                    '--rc', '--rc-addr', '127.0.0.1:5572'], check=True)
assert os.path.ismount(drive)
"#;

pub async fn mount_google_drive_with_consent<F, Fut>(
    state: &Path,
    name: &str,
    mode: DriveMountMode,
    rclone_config: Option<&str>,
    consent: F,
) -> Result<(), String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    match mode {
        DriveMountMode::Disabled => return Ok(()),
        DriveMountMode::ColabNative => native_drive(state, name, consent).await?,
        DriveMountMode::RcloneUnattended => {
            let config = rclone_config
                .filter(|s| !s.is_empty())
                .ok_or("Missing rclone configuration.")?;
            exec_snippet(state, name, "from pathlib import Path\nPath('/root/.notebookx').mkdir(mode=0o700,exist_ok=True)").await?;
            let mut cmd = scoped_cli(state).await?;
            cmd.args(["_upload-rclone", "-s", name])
                .env("NBX_RCLONE_CONFIG", config);
            if !run(cmd, None, 90).await?.status.success() {
                return Err("Could not transfer rclone configuration.".into());
            }
            exec_snippet(state, name, RCLONE_MOUNT).await?;
        }
    }
    exec_snippet(
        state,
        name,
        "import os\nassert os.path.isdir('/content/drive/MyDrive'), 'Drive mount is missing'",
    )
    .await?;
    Ok(())
}

pub struct MountResult {
    pub auth_url: Option<String>,
}

pub async fn mount_google_drive(
    state: &Path,
    name: &str,
    mode: DriveMountMode,
    rclone_config: Option<&str>,
) -> Result<MountResult, String> {
    let mut auth_url = None;
    mount_google_drive_with_consent(state, name, mode, rclone_config, |url| {
        auth_url = Some(url);
        async {
            let account = account_id(state)?;
            let endpoint = session_endpoint(state, name)
                .await?
                .ok_or("Drive session disappeared.")?;
            loop {
                sleep(Duration::from_secs(3)).await;
                let result =
                    account_helper(&["drive-consent-ready", account, &endpoint], None).await?;
                if result["ready"] == true {
                    return Ok(());
                }
            }
        }
    })
    .await?;
    Ok(MountResult { auth_url })
}

fn literal(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}

pub async fn deploy_watchdog_with_policy(
    state: &Path,
    name: &str,
    deadline_epoch: Option<i64>,
    idle_timeout_s: Option<u64>,
    backup_command: Option<&str>,
) -> Result<(), String> {
    let config = serde_json::json!({ "deadline_epoch": deadline_epoch, "idle_timeout_s": idle_timeout_s, "backup_command": backup_command });
    let code = format!(
        r#"
from pathlib import Path
import os, subprocess, sys
root = Path('/root/.notebookx')
root.mkdir(mode=0o700, exist_ok=True)
def nbx_write(name, text):
    import tempfile
    with tempfile.NamedTemporaryFile(mode='w', dir=root, delete=False) as f:
        f.write(text)
    os.replace(f.name, root / name)
nbx_write('watchdog.py', {})
nbx_write('watchdog.json', {})
with (root/'watchdog.log').open('a') as log:
    subprocess.Popen([sys.executable, str(root/'watchdog.py')], stdin=subprocess.DEVNULL,
                     stdout=log, stderr=log, start_new_session=True, close_fds=True)
import fcntl, time
with (root/'watchdog.lock').open('a') as lock:
    for _ in range(50):
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            fcntl.flock(lock, fcntl.LOCK_UN)
        except BlockingIOError:
            break
        time.sleep(0.1)
    else:
        raise RuntimeError('Watchdog did not start')
"#,
        literal(ASSETS_WATCHDOG_PY),
        literal(&config.to_string())
    );
    exec_snippet(state, name, &code).await.map(|_| ())
}

pub async fn deploy_watchdog(
    state: &Path,
    name: &str,
    auto_minutes: Option<u32>,
    idle_minutes: Option<u32>,
) -> Result<(), String> {
    let backup =
        format!("python3 - <<'NBX_BACKUP'\n{MANAGEMENT_BACKUP}\n{DRIVE_BACKUP}\nNBX_BACKUP");
    deploy_watchdog_with_policy(
        state,
        name,
        auto_minutes.map(|m| chrono::Utc::now().timestamp() + i64::from(m) * 60),
        idle_minutes.map(|m| u64::from(m) * 60),
        Some(&backup),
    )
    .await
}

pub async fn update_watchdog_deadline(
    state: &Path,
    name: &str,
    deadline_epoch: Option<f64>,
    idle_minutes: Option<u32>,
) -> Result<(), String> {
    if deadline_epoch.is_some_and(|s| !s.is_finite()) {
        return Err("Invalid watchdog deadline.".into());
    }
    let code = format!(
        r#"
import json, os, tempfile
from pathlib import Path
cfg = Path('/root/.notebookx/watchdog.json')
data = json.loads(cfg.read_text())
data['deadline_epoch'] = {}
{}
with tempfile.NamedTemporaryFile(mode='w', dir=cfg.parent, delete=False) as f:
    json.dump(data, f)
os.replace(f.name, cfg)
"#,
        deadline_epoch.map_or_else(|| "None".into(), |x| x.to_string()),
        idle_minutes.map_or_else(String::new, |m| format!(
            "data['idle_timeout_s'] = {}",
            u64::from(m) * 60
        ))
    );
    exec_snippet(state, name, &code).await.map(|_| ())
}

const QUEUE_CHECK: &str = r#"
import errno, json, os, urllib.error, urllib.request
try:
    port = int(os.environ.get('NBX_COMFY_PORT', 8188))
    with urllib.request.urlopen(f'http://127.0.0.1:{port}/queue', timeout=5) as r:
        q = json.load(r)
    assert all(isinstance(q[k], list) for k in ('queue_running','queue_pending'))
    print('__NBX_QUEUE__', 'busy' if q['queue_running'] or q['queue_pending'] else 'idle')
except urllib.error.URLError as err:
    if getattr(err.reason, 'errno', None) != errno.ECONNREFUSED:
        raise
    print('__NBX_QUEUE__ idle')
"#;

const MANAGEMENT_BACKUP: &str = r#"
import errno, os, urllib.request, urllib.error
try:
    port = int(os.environ.get('NBX_MGMT_PORT', 8189))
    with urllib.request.urlopen(f'http://127.0.0.1:{port}/backup', data=b'', timeout=600) as r:
        r.read()
except urllib.error.HTTPError as err:
    if err.code not in (404, 405): raise
except urllib.error.URLError as err:
    if getattr(err.reason, 'errno', None) != errno.ECONNREFUSED: raise
"#;

const DRIVE_BACKUP: &str = r#"
from pathlib import Path
import os, subprocess
src = Path('/content/ComfyUI/output')
if src.is_dir() and any(src.iterdir()):
    drive = Path('/content/drive/MyDrive')
    assert drive.is_dir() and (os.path.ismount(drive) or os.path.ismount(drive.parent)), 'Drive is unavailable'
    dest = Path(os.environ.get('NBX_DRIVE_ROOT', drive / 'NotebookX')) / 'outputs'
    remote_path = dest.resolve().relative_to(drive.resolve()).as_posix()
    if os.path.ismount(drive) and Path('/root/.notebookx/rclone.conf').is_file():
        import configparser
        cfg = '/root/.notebookx/rclone.conf'
        ini = configparser.ConfigParser(interpolation=None)
        ini.read(cfg)
        remote = next(s for s in ini.sections() if ini.get(s, 'type', fallback='') == 'drive')
        if src.resolve() != dest.resolve():
            subprocess.run(['rclone', '--config', cfg, 'copy', str(src), remote + ':' + remote_path], check=True)
    elif src.resolve() != dest.resolve():
        dest.mkdir(parents=True, exist_ok=True)
        subprocess.run(['rsync', '-a', str(src) + '/', str(dest) + '/'], check=True)
    os.sync()
if os.path.ismount('/content/drive/MyDrive') and Path('/root/.notebookx/rclone.conf').is_file():
    import json, time
    end = time.monotonic() + 600
    while True:
        stats = json.loads(subprocess.check_output(['rclone', 'rc', 'vfs/stats']))['diskCache']
        if stats['erroredFiles']:
            raise RuntimeError('Drive has failed uploads')
        if stats['uploadsQueued'] == 0 and stats['uploadsInProgress'] == 0:
            break
        if time.monotonic() >= end:
            raise TimeoutError('Drive uploads are still pending')
        time.sleep(2)
"#;

fn local_url(base: &str, path: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(base).map_err(|_| "Invalid tunnel URL.")?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
    {
        return Err("Expected a loopback tunnel URL.".into());
    }
    url.set_path(path);
    url.set_query(None);
    Ok(url)
}

async fn ensure_idle(state: &Path, name: &str, local: Option<&str>) -> Result<(), String> {
    if let Some(local) = local {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(4))
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;
        if let Ok(response) = http.get(local_url(local, "/queue")?).send().await {
            let queue: serde_json::Value = response
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())?;
            let running = queue["queue_running"]
                .as_array()
                .ok_or("Invalid ComfyUI queue.")?;
            let pending = queue["queue_pending"]
                .as_array()
                .ok_or("Invalid ComfyUI queue.")?;
            return if running.is_empty() && pending.is_empty() {
                Ok(())
            } else {
                Err("ComfyUI has running or pending work.".into())
            };
        }
    }
    let result = exec_snippet(state, name, QUEUE_CHECK).await?;
    if result.lines().any(|l| l.trim() == "__NBX_QUEUE__ idle") {
        Ok(())
    } else {
        Err("ComfyUI has running or pending work. Stop deferred.".into())
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn verified_safe_stop(
    _app_data: &Path,
    state: &Path,
    id: &str,
    name: &str,
    endpoint: &str,
    drive_root: Option<&str>,
    comfy_url: Option<&str>,
    management_url: Option<&str>,
    safe_backup: bool,
    force: bool,
) -> Result<bool, String> {
    if endpoint.is_empty() {
        return Err("An endpoint is required for verified release.".into());
    }
    if account_id(state)? != id {
        return Err("Account and CLI state do not match.".into());
    }
    let _guard = session_lock(state, name).await;
    if matches!(assigned(state, endpoint).await, Ok(false)) {
        forget_released(state, name, endpoint).await?;
        return Ok(true);
    }
    if session_endpoint(state, name).await?.as_deref() == Some(endpoint) {
        if !force {
            ensure_idle(state, name, comfy_url).await?;
        }
        if safe_backup {
            let mut mgmt = MANAGEMENT_BACKUP;
            if let Some(base) = management_url {
                let http = reqwest::Client::builder()
                    .timeout(Duration::from_secs(600))
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .map_err(|e| e.to_string())?;
                if let Ok(response) = http.post(local_url(base, "/backup")?).send().await {
                    if response.status().is_success() {
                        mgmt = "";
                    } else if !matches!(response.status().as_u16(), 404 | 405) {
                        return Err("Management backup failed.".into());
                    }
                }
            }
            let env = drive_root
                .map(|root| vec![("NBX_DRIVE_ROOT".into(), root.to_owned())])
                .unwrap_or_default();
            let backup = format!("{mgmt}\n{DRIVE_BACKUP}");
            exec_python_on_session(state, name, &backup, &env, 900)
                .await
                .map_err(|e| format!("Pre-stop backup failed. {e}"))?;
        }
        if !force {
            ensure_idle(state, name, comfy_url).await?;
        }
    } else if !force {
        return Err(
            "Orphan assignment. Force is required because its queue cannot be inspected.".into(),
        );
    }
    release(state, name, endpoint).await.map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_args_and_guard_boundary() {
        assert_eq!(
            new_args("test", AcceleratorKind::A100, true),
            ["new", "-s", "test", "--gpu", "A100", "--high-mem"]
        );
        assert_eq!(
            new_args("cpu", AcceleratorKind::Cpu, false),
            ["new", "-s", "cpu"]
        );
        assert!(!sufficient_vram(40_960, 80));
        assert!(!sufficient_vram(80_895, 80));
        assert!(sufficient_vram(80_896, 80));
        assert!(sufficient_vram(0, 0));
        assert_eq!(parse_gpu("NVIDIA A100, 81251, 55").unwrap().1, 81251);
    }
}
