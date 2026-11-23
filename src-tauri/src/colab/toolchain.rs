use crate::types::{BackendMode, EnvironmentStatus};
use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{process::Command, time::timeout};

pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd.kill_on_drop(true)
        .stdin(Stdio::null())
        .env("PYTHONUTF8", "1");
    cmd
}

async fn output(program: &Path, args: &[&str], seconds: u64) -> Option<String> {
    let out = timeout(
        Duration::from_secs(seconds),
        command(program).args(args).output(),
    )
    .await
    .ok()?
    .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn executable(dir: &Path, name: &str) -> PathBuf {
    dir.join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    })
}

fn candidates(name: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = env::var_os("PATH")
        .into_iter()
        .flat_map(|p| env::split_paths(&p).collect::<Vec<_>>())
        .collect();
    if let Some(home) = dirs::home_dir() {
        dirs.extend([home.join(".local/bin"), home.join(".cargo/bin")]);
    }
    #[cfg(unix)]
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    #[cfg(windows)]
    {
        if let Some(root) = env::var_os("SystemRoot") {
            dirs.insert(0, PathBuf::from(root).join("System32/OpenSSH"));
        }
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft/WinGet/Links"));
        }
        for (var, subdir) in [("APPDATA", "Python"), ("LOCALAPPDATA", "Programs/Python")] {
            if let Some(base) = env::var_os(var).map(PathBuf::from).map(|p| p.join(subdir)) {
                dirs.push(base.join("Scripts"));
                if let Ok(entries) = std::fs::read_dir(base) {
                    for entry in entries.flatten() {
                        dirs.extend([entry.path(), entry.path().join("Scripts")]);
                    }
                }
            }
        }
    }
    dirs.into_iter()
        .map(|d| executable(&d, name))
        .filter(|p| p.is_file())
        .collect()
}

pub(crate) fn find_ssh() -> Result<PathBuf, String> {
    candidates("ssh")
        .into_iter()
        .next()
        .ok_or_else(|| "OpenSSH client was not found.".into())
}

pub(crate) async fn python() -> Option<PathBuf> {
    installed_python(9).await
}

async fn installed_python(min_minor: u8) -> Option<PathBuf> {
    let probe =
        format!("import sys; assert sys.version_info >= (3,{min_minor}); print(sys.executable)");
    for path in candidates("python3")
        .into_iter()
        .chain(candidates("python"))
        .chain(candidates("py"))
    {
        if let Some(actual) = output(&path, &["-c", &probe], 5).await {
            let path = PathBuf::from(actual);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    uv_dir(&[
        "python",
        "find",
        "--no-python-downloads",
        &format!(">=3.{min_minor}"),
    ])
    .await
    .filter(|p| p.is_file())
}

async fn uv_dir(args: &[&str]) -> Option<PathBuf> {
    let uv = candidates("uv").into_iter().next()?;
    let value = output(&uv, args, 8).await?;
    (!value.is_empty()).then(|| PathBuf::from(value))
}

async fn find_colab() -> Option<PathBuf> {
    if let Some(p) = candidates("colab").into_iter().next() {
        return Some(p);
    }
    if let Some(dir) = uv_dir(&["tool", "dir", "--bin"]).await {
        let p = executable(&dir, "colab");
        if p.is_file() {
            return Some(p);
        }
    }
    let dir = uv_dir(&["tool", "dir"]).await?.join("google-colab-cli");
    let p = executable(
        &dir.join(if cfg!(windows) { "Scripts" } else { "bin" }),
        "colab",
    );
    p.is_file().then_some(p)
}

async fn launcher_python(cli: &Path) -> Option<PathBuf> {
    let bytes = tokio::fs::read(cli).await.ok()?;
    // pip's Windows .exe launcher contains a shebang just before its ZIP payload.
    let end = bytes
        .windows(4)
        .position(|v| v == b"PK\x03\x04")
        .unwrap_or(bytes.len());
    let start = bytes[..end].windows(2).rposition(|v| v == b"#!")? + 2;
    let line = std::str::from_utf8(bytes[start..end].split(|b| *b == b'\n').next()?)
        .ok()?
        .trim()
        .trim_matches('"');
    let path = PathBuf::from(line);
    path.is_file().then_some(path)
}

pub(crate) async fn cli_python() -> Result<PathBuf, String> {
    static FOUND: tokio::sync::Mutex<Option<PathBuf>> = tokio::sync::Mutex::const_new(None);
    let mut cached = FOUND.lock().await;
    if let Some(p) = cached.as_ref().filter(|p| p.is_file()) {
        return Ok(p.clone());
    }
    let mut possible = Vec::new();
    if let Some(cli) = find_colab().await {
        let real = std::fs::canonicalize(&cli).unwrap_or(cli);
        if let Some(p) = launcher_python(&real).await {
            possible.push(p);
        }
        if let Some(parent) = real.parent() {
            possible.push(executable(parent, "python"));
        }
    }
    if let Some(dir) = uv_dir(&["tool", "dir"]).await {
        possible.push(executable(
            &dir.join("google-colab-cli")
                .join(if cfg!(windows) { "Scripts" } else { "bin" }),
            "python",
        ));
    }
    if let Some(p) = python().await {
        possible.push(p);
    }
    for p in possible {
        if p.is_file()
            && output(
                &p,
                &[
                    "-c",
                    "import importlib.util as u; assert u.find_spec('colab_cli')",
                ],
                6,
            )
            .await
            .is_some()
        {
            *cached = Some(p.clone());
            return Ok(p);
        }
    }
    Err("Could not locate the Python environment containing google-colab-cli.".into())
}

pub async fn install_colab_cli() -> Result<String, String> {
    if let Some(uv) = candidates("uv").into_iter().next() {
        // Let uv select/download a native compatible interpreter even when the
        // desktop's default Python is older than the CLI requires (>=3.12).
        if output(
            &uv,
            &["tool", "install", "--python", "3.13", "google-colab-cli"],
            600,
        )
        .await
        .is_some()
        {
            return Ok("Installed google-colab-cli with uv.".into());
        }
    }
    let py = installed_python(12)
        .await
        .ok_or("Install Python 3.12 or newer, or uv, to install the Colab CLI.")?;
    output(&py, &["-m", "pip", "install", "--user", "google-colab-cli"], 600).await
        .map(|_| "Installed google-colab-cli with pip.".into())
        .ok_or_else(|| "pip could not install google-colab-cli. It requires Python 3.12 or newer. Check network and user-site permissions.".into())
}

fn key_path() -> Result<PathBuf, String> {
    Ok(dirs::home_dir()
        .ok_or("Home directory unavailable.")?
        .join(".ssh/id_ed25519"))
}

pub async fn ensure_ssh_key() -> Result<PathBuf, String> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = LOCK.lock().await;
    let key = key_path()?;
    let public = key.with_extension("pub");
    if key.is_file() && public.is_file() {
        return Ok(key);
    }
    let tool = candidates("ssh-keygen")
        .into_iter()
        .next()
        .ok_or("ssh-keygen is unavailable.")?;
    let directory = key.parent().unwrap();
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|e| e.to_string())?;
    if key.exists() {
        let out = command(tool)
            .args(["-y", "-P", "", "-f"])
            .arg(&key)
            .output()
            .await
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err("Unable to read the existing SSH identity.".into());
        }
        tokio::fs::write(public, out.stdout)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        if public.exists() {
            return Err("The SSH public key exists but its private key is missing.".into());
        }
        let status = command(tool)
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&key)
            .output()
            .await
            .map_err(|e| e.to_string())?
            .status;
        if !status.success() {
            return Err("Could not generate an SSH identity.".into());
        }
    }
    Ok(key)
}

pub async fn detect_environment(
    mode: BackendMode,
    gateway: bool,
    gateway_url: String,
    drama: bool,
    drama_url: String,
) -> EnvironmentStatus {
    let (py, cli) = tokio::join!(python(), find_colab());
    let uv = candidates("uv").into_iter().next();
    let ssh = find_ssh().ok();
    let key = key_path().ok();
    let version = match &cli {
        Some(cli) => output(cli, &["version"], 12).await,
        None => None,
    };
    let display = |p: PathBuf| p.to_string_lossy().into_owned();
    EnvironmentStatus {
        backendMode: mode,
        pythonFound: py.is_some(),
        pythonPath: py.map(display),
        uvFound: uv.is_some(),
        uvPath: uv.map(display),
        colabCliFound: cli.is_some(),
        colabCliPath: cli.map(display),
        colabCliVersion: version,
        sshFound: ssh.is_some(),
        sshPath: ssh.map(display),
        sshKeyReady: key
            .as_ref()
            .is_some_and(|p| p.is_file() && p.with_extension("pub").is_file()),
        sshKeyPath: key.map(display),
        gatewayRunning: gateway,
        gatewayUrl: gateway_url,
        dramaSmithReachable: drama,
        dramaSmithUrl: drama_url,
    }
}
