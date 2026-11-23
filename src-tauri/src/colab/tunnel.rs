use super::{
    accounts::{scoped_cli, session_endpoint},
    toolchain::{command, ensure_ssh_key, find_ssh},
};
use crate::types::{PortTunnel, TunnelStatus};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    net::TcpStream,
    sync::{oneshot, watch, Mutex},
    task::JoinHandle,
    time::{sleep, timeout},
};
use uuid::Uuid;

pub struct TunnelHandle {
    state: watch::Receiver<PortTunnel>,
    cancel: std::sync::Mutex<Option<oneshot::Sender<()>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl TunnelHandle {
    pub fn snapshot(&self) -> PortTunnel {
        self.state.borrow().clone()
    }

    pub async fn stop(&self) {
        if let Some(sender) = self.cancel.lock().unwrap().take() {
            let _ = sender.send(());
        }
        let mut task = self.task.lock().await;
        if let Some(job) = task.take() {
            let _ = job.await;
        }
    }
}

fn quote_proxy_arg(s: &str, windows: bool) -> Result<String, String> {
    if s.contains(['\0', '\n', '\r']) {
        return Err("Invalid SSH proxy argument.".into());
    }
    if windows {
        if s.contains('"') {
            return Err("Unexpected quote in Windows SSH proxy argument.".into());
        }
        // Win32 OpenSSH launches the proxy directly. Percent expansion still
        // occurs in OpenSSH before the command reaches CreateProcess.
        Ok(format!("\"{}\"", s.replace('%', "%%")))
    } else {
        Ok(format!("'{}'", s.replace('\'', "'\\''").replace('%', "%%")))
    }
}

fn ssh_args(
    proxy: &[String],
    key: &Path,
    local: u16,
    remote: u16,
    windows: bool,
) -> Result<Vec<String>, String> {
    let null = if windows { "NUL" } else { "/dev/null" };
    let proxy = proxy
        .iter()
        .map(|s| quote_proxy_arg(s, windows))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    Ok(vec![
        "-N".into(),
        "-T".into(),
        "-F".into(),
        null.into(),
        "-o".into(),
        "ExitOnForwardFailure=yes".into(),
        "-o".into(),
        "StrictHostKeyChecking=no".into(),
        "-o".into(),
        format!("UserKnownHostsFile={null}"),
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "IdentitiesOnly=yes".into(),
        "-o".into(),
        "ConnectTimeout=15".into(),
        "-o".into(),
        "ServerAliveInterval=15".into(),
        "-o".into(),
        "ServerAliveCountMax=2".into(),
        "-o".into(),
        format!("ProxyCommand={proxy}"),
        "-i".into(),
        key.to_string_lossy().into_owned(),
        "-L".into(),
        format!("127.0.0.1:{local}:127.0.0.1:{remote}"),
        "root@colab-runtime".into(),
    ])
}

pub async fn start_tunnel(
    state: &Path,
    session: &str,
    local_port: u16,
    remote_port: u16,
    health_path: Option<&str>,
) -> Result<TunnelHandle, String> {
    if local_port == 0 || remote_port == 0 {
        return Err("Specify a nonzero TCP port.".into());
    }
    if health_path.is_some_and(|s| !s.starts_with('/') || s.contains(['\r', '\n'])) {
        return Err("Invalid HTTP health path.".into());
    }
    let endpoint = session_endpoint(state, session)
        .await?
        .ok_or("Cannot tunnel to a missing session.")?;
    let ssh = find_ssh()?;
    let key = ensure_ssh_key().await?;
    let mut cli = scoped_cli(state).await?;
    cli.args(["ssh", "--proxy-mode", "-s", session, "--identity"])
        .arg(&key);
    let cli = cli.as_std();
    let proxy = std::iter::once(cli.get_program())
        .chain(cli.get_args())
        .map(|s| {
            s.to_str()
                .map(str::to_owned)
                .ok_or("SSH proxy requires a Unicode path.")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let args = ssh_args(&proxy, &key, local_port, remote_port, cfg!(windows))?;
    let url = health_path.map(|p| format!("http://127.0.0.1:{local_port}{p}"));
    let http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    let (tx, rx) = watch::channel(PortTunnel {
        id: Uuid::new_v4().to_string(),
        sessionName: session.into(),
        remotePort: remote_port,
        localPort: local_port,
        label: format!("{session}:{remote_port}"),
        status: TunnelStatus::Connecting,
        healthy: None,
        healthPath: health_path.map(str::to_owned),
        reconnectCount: 0,
        lastError: None,
    });
    let (cancel, mut stop) = oneshot::channel();
    let state = state.to_owned();
    let session = session.to_owned();
    let task = tokio::spawn(async move {
        let mut delay = 1;
        'supervise: loop {
            if matches!(session_endpoint(&state, &session).await, Ok(record) if record.as_ref() != Some(&endpoint))
            {
                break;
            }
            let child = command(&ssh)
                .args(&args)
                .env("NBX_TUNNEL_ENDPOINT", &endpoint)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            if let Ok(mut child) = child {
                loop {
                    tokio::select! {
                        biased;
                        _ = &mut stop => { let _ = child.kill().await; break 'supervise; }
                        _ = child.wait() => break,
                        _ = sleep(Duration::from_secs(2)) => {
                            let bound = matches!(timeout(Duration::from_secs(1), TcpStream::connect(("127.0.0.1", local_port))).await, Ok(Ok(_)));
                            let healthy = match &url {
                                Some(url) if bound => Some(http.get(url).send().await.is_ok_and(|r| r.status().is_success())),
                                Some(_) => Some(false),
                                None => None,
                            };
                            if bound { delay = 1; }
                            tx.send_modify(|s| {
                                s.healthy = healthy;
                                if bound { s.status = TunnelStatus::Connected; s.lastError = None; }
                            });
                        }
                    }
                }
            }
            tx.send_modify(|s| {
                s.status = TunnelStatus::Reconnecting;
                s.healthy = Some(false);
                s.reconnectCount = s.reconnectCount.saturating_add(1);
                s.lastError = Some(format!("SSH exited. Reconnecting in {delay}s."));
            });
            tokio::select! {
                _ = &mut stop => break,
                _ = sleep(Duration::from_secs(delay)) => {}
            }
            delay = (delay * 2).min(15);
        }
        tx.send_modify(|s| {
            s.status = TunnelStatus::Stopped;
            s.healthy = Some(false);
        });
    });
    Ok(TunnelHandle {
        state: rx,
        cancel: std::sync::Mutex::new(Some(cancel)),
        task: Mutex::new(Some(task)),
    })
}

pub async fn spawn_supervised_tunnel(
    state: std::path::PathBuf,
    session: String,
    remote_port: u16,
    local_port: u16,
    health: Option<String>,
) -> Result<TunnelHandle, String> {
    start_tunnel(&state, &session, local_port, remote_port, health.as_deref()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_quoting_and_loopback_are_platform_specific() {
        let proxy = [
            "colab",
            "--config",
            "with spaces/state.json",
            "ssh",
            "--proxy-mode",
            "-s",
            "abc",
        ]
        .map(str::to_owned);
        for (windows, null) in [(true, "NUL"), (false, "/dev/null")] {
            let args = ssh_args(&proxy, Path::new("identity"), 1234, 8188, windows).unwrap();
            assert!(args.windows(2).any(|v| v == ["-F", null]));
            assert!(args.contains(&format!("UserKnownHostsFile={null}")));
            assert!(args.contains(&"127.0.0.1:1234:127.0.0.1:8188".into()));
            let quoted = if windows {
                "\"with spaces/state.json\""
            } else {
                "'with spaces/state.json'"
            };
            assert!(args
                .iter()
                .any(|s| s.starts_with("ProxyCommand=") && s.contains(quoted)));
        }
    }
}
