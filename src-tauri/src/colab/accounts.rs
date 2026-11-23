use super::toolchain::{cli_python, command, python};
use crate::types::{AcceleratorKind, GoogleAccount};
use serde::Deserialize;
use serde_json::Value;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{fs, io::AsyncWriteExt, process::Command, time::timeout};
use uuid::Uuid;

const HELPER: &str = include_str!("../../assets/account_helper.py");

fn valid_id(id: &str) -> bool {
    let stem = id.split('.').next().unwrap_or("").to_ascii_uppercase();
    !id.is_empty()
        && id.len() <= 128
        && id != "active"
        && !id.starts_with('.')
        && !id.ends_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_@.".contains(&b))
        && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
}

pub fn account_state_path(app_data: &Path, account_id: &str) -> Result<PathBuf, String> {
    if !valid_id(account_id) {
        return Err("Invalid account ID.".into());
    }
    let directory = app_data.join("accounts").join(account_id);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    Ok(directory.join("colab_state.json"))
}

pub fn account_state_file(app_data: &Path, id: &str) -> PathBuf {
    // Keep the infallible UI path accessor. An invalid ID produces an unusable
    // path that scoped_cli rejects before spawning any command.
    if valid_id(id) {
        app_data.join("accounts").join(id).join("colab_state.json")
    } else {
        PathBuf::new()
    }
}

pub(crate) fn account_id(state: &Path) -> Result<&str, String> {
    state
        .parent()
        .and_then(Path::file_name)
        .and_then(|p| p.to_str())
        .filter(|s| valid_id(s))
        .ok_or_else(|| "The Colab state file must belong to an account directory.".into())
}

async fn helper_path() -> Result<PathBuf, String> {
    // Immutable, versioned path: an upgrade cannot replace a running proxy script.
    let mut hash = DefaultHasher::new();
    HELPER.hash(&mut hash);
    let dir = dirs::cache_dir()
        .ok_or("User cache directory unavailable.")?
        .join("notebookx-desktop");
    fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let path = dir.join(format!("account_helper_{:x}.py", hash.finish()));
    if fs::read(&path).await.is_ok_and(|b| b == HELPER.as_bytes()) {
        return Ok(path);
    }
    let temporary = dir.join(format!("{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, HELPER)
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = fs::rename(&temporary, &path).await {
        let _ = fs::remove_file(&temporary).await;
        if !fs::read(&path).await.is_ok_and(|b| b == HELPER.as_bytes()) {
            return Err(e.to_string());
        }
    }
    Ok(path)
}

pub(crate) async fn scoped_cli(state: &Path) -> Result<Command, String> {
    let id = account_id(state)?;
    fs::create_dir_all(state.parent().unwrap())
        .await
        .map_err(|e| e.to_string())?;
    let (py, helper) = tokio::try_join!(cli_python(), helper_path())?;
    let mut cmd = command(py);
    cmd.arg(helper)
        .arg("_cli")
        .arg(id)
        .arg("--config")
        .arg(state);
    Ok(cmd)
}

pub(crate) async fn run(
    mut cmd: Command,
    input: Option<&str>,
    seconds: u64,
) -> Result<std::process::Output, String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pipe = child.stdin.take();
    timeout(Duration::from_secs(seconds), async {
        let send = async {
            if let (Some(mut pipe), Some(input)) = (pipe, input) {
                pipe.write_all(input.as_bytes()).await?;
                pipe.shutdown().await?;
            }
            Ok::<(), std::io::Error>(())
        };
        let ((), result) =
            tokio::try_join!(send, child.wait_with_output()).map_err(|e| e.to_string())?;
        Ok(result)
    })
    .await
    .map_err(|_| format!("The operation timed out after {seconds}s."))?
}

pub async fn account_helper(args: &[&str], stdin: Option<&str>) -> Result<Value, String> {
    let op = args.first().copied().ok_or("Missing account operation.")?;
    if op.starts_with('_') {
        return Err("Invalid account operation.".into());
    }
    let py = match cli_python().await {
        Ok(py) => py,
        Err(_) => python().await.ok_or("Python is unavailable.")?,
    };
    let mut cmd = command(py);
    cmd.arg(helper_path().await?).args(args);
    let result = run(cmd, stdin, 120).await?;
    let stdout =
        std::str::from_utf8(&result.stdout).map_err(|_| "Invalid credential helper output.")?;
    let line = stdout
        .lines()
        .find_map(|l| l.strip_prefix("__NBX_RESULT__ "))
        .ok_or("Credential helper did not return a result.")?;
    let value: Value =
        serde_json::from_str(line).map_err(|_| "Invalid credential helper result.")?;
    if !result.status.success() {
        return Err(value["error"]
            .as_str()
            .unwrap_or("Credential operation failed.")
            .into());
    }
    Ok(value)
}

pub async fn secret_has(_app_data: &Path, name: &str) -> Result<bool, String> {
    account_helper(&["secret-has", name], None).await?["exists"]
        .as_bool()
        .ok_or("Invalid secret lookup.".into())
}

pub async fn secret_get(_app_data: &Path, name: &str) -> Result<Option<String>, String> {
    Ok(account_helper(&["secret-get", name], None).await?["value"]
        .as_str()
        .map(str::to_owned))
}

pub async fn secret_set(_app_data: &Path, name: &str, value: &str) -> Result<(), String> {
    account_helper(&["secret-set", name], Some(value))
        .await
        .map(|_| ())
}

pub async fn secret_delete(_app_data: &Path, name: &str) -> Result<(), String> {
    account_helper(&["secret-delete", name], None)
        .await
        .map(|_| ())
}

pub async fn start_oauth_login(
    _app_data: &Path,
    _existing_ids: &[String],
) -> Result<String, String> {
    let url = account_helper(&["begin-login"], None).await?["url"]
        .as_str()
        .map(str::to_owned)
        .ok_or("Missing Google consent URL.")?;
    let _ = open::that(&url);
    Ok(url)
}

fn account(id: &str, reply: Value) -> Result<GoogleAccount, String> {
    Ok(GoogleAccount {
        id: id.into(),
        email: reply["email"]
            .as_str()
            .ok_or("Missing account email.")?
            .into(),
        displayName: None,
        isActive: true,
        planTier: "UNKNOWN".into(),
        balanceCcu: None,
        burnRateHourly: None,
        hoursRemaining: None,
        activeAssignmentsCount: 0,
        lastUpdatedAt: None,
        usageStale: true,
    })
}

pub async fn complete_oauth_login(
    app_data: &Path,
    id: &str,
    code: Option<String>,
) -> Result<GoogleAccount, String> {
    account_state_path(app_data, id)?;
    let code = code
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("Enter the authorization code from Google.")?;
    let reply = account_helper(&["complete-login", id], Some(code))
        .await
        .map_err(|e| format!("{e}. Start sign-in again."))?;
    account(id, reply)
}

pub async fn adopt_active_cli_account(app_data: &Path, id: &str) -> Result<GoogleAccount, String> {
    account_state_path(app_data, id)?;
    account(id, account_helper(&["adopt", id], None).await?)
}

pub async fn use_saved_account(_app_data: &Path, id: &str) -> Result<(), String> {
    account_helper(&["use", id], None).await.map(|_| ())
}
pub async fn remove_saved_account(_app_data: &Path, id: &str) -> Result<(), String> {
    account_helper(&["remove", id], None).await.map(|_| ())
}

#[derive(Deserialize)]
pub struct AccountUsage {
    #[serde(rename = "currentBalance")]
    pub balance_ccu: f64,
    #[serde(rename = "consumptionRateHourly")]
    pub burn_rate_hourly: f64,
    #[serde(rename = "assignmentsCount")]
    pub assignments_count: u32,
    #[serde(rename = "planTier")]
    pub plan_tier: String,
}

pub async fn fetch_account_usage(_app_data: &Path, id: &str) -> Result<AccountUsage, String> {
    serde_json::from_value(account_helper(&["usage", id], None).await?).map_err(|e| e.to_string())
}

pub struct ActiveAssignment {
    pub endpoint: String,
    pub accelerator: AcceleratorKind,
    pub high_mem: bool,
    pub variant: Value,
    pub machine_shape: Value,
}

pub async fn list_active_assignments(
    _app_data: &Path,
    id: &str,
) -> Result<Vec<ActiveAssignment>, String> {
    let rows = account_helper(&["assignments", id], None).await?;
    rows.as_array()
        .ok_or("Invalid assignments response.")?
        .iter()
        .map(|row| {
            let accelerator = if row["accelerator"] == "NONE" {
                AcceleratorKind::Cpu
            } else {
                serde_json::from_value(row["accelerator"].clone())
                    .map_err(|_| "Unknown assignment accelerator.")?
            };
            Ok(ActiveAssignment {
                endpoint: row["endpoint"]
                    .as_str()
                    .ok_or("Missing assignment endpoint.")?
                    .into(),
                accelerator,
                high_mem: row["machineShape"] == 1 || row["machineShape"] == "HIGH_RAM",
                variant: row["variant"].clone(),
                machine_shape: row["machineShape"].clone(),
            })
        })
        .collect()
}

pub(crate) async fn session_endpoint(state: &Path, name: &str) -> Result<Option<String>, String> {
    // Upstream StateStore rewrites its file in place under a Python lock.
    // Another session can be writing while Rust inspects this account's map.
    let mut attempts = 0;
    let records: Value = loop {
        let bytes = match fs::read(state).await {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        if let Ok(records) = serde_json::from_slice(&bytes) {
            break records;
        }
        attempts += 1;
        if attempts == 5 {
            return Err("Could not read the CLI session state.".into());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    let records = records.as_object().ok_or("Invalid CLI session state.")?;
    records
        .get(name)
        .map(|s| {
            s["endpoint"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| "No endpoint in CLI session record.".into())
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_paths_are_isolated() {
        let dir = tempfile::tempdir().unwrap();
        let a = account_state_path(dir.path(), "abc").unwrap();
        assert_eq!(account_id(&a).unwrap(), "abc");
        assert_ne!(a, account_state_path(dir.path(), "def").unwrap());
        for bad in ["..", "C:other", "CON.txt", "a/b", "active"] {
            assert!(account_state_path(dir.path(), bad).is_err());
        }
    }
}
