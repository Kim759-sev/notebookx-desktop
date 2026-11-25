pub mod accounts;
pub mod session_runner;
pub mod toolchain;
pub mod tunnel;

pub use accounts::{
    account_helper, account_state_path, adopt_active_cli_account, complete_oauth_login,
    fetch_account_usage, list_active_assignments, remove_saved_account, start_oauth_login,
    use_saved_account,
};
pub use session_runner::{
    deploy_watchdog, deploy_watchdog_with_policy, exec_python_on_session, mount_google_drive,
    mount_google_drive_notifying, mount_google_drive_with_consent, provision_with_vram_guard,
    update_watchdog_deadline, verified_safe_stop, MountResult, ProvisionedSession,
    ASSETS_WATCHDOG_PY,
};
pub use toolchain::{detect_environment, ensure_ssh_key, install_colab_cli};
pub use tunnel::{start_tunnel, TunnelHandle};

#[cfg(test)]
mod tests {
    use super::toolchain::{command, python};
    use std::path::Path;

    #[tokio::test]
    async fn offline_helper_xssi_vault_and_login_rollback() {
        let py = python().await.expect("Python 3 required for helper tests");
        let root = tempfile::tempdir().unwrap();
        let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/account_helper.py");
        let script = r#"
import json, runpy, sys
from pathlib import Path
sys.modules['keyring'] = None
h = runpy.run_path(sys.argv[1])
g = h['dispatch'].__globals__
root = Path(sys.argv[2])
g.update(ROOT=root, VAULT=root/'vault.json', LEGACY=root/'cli'/'token.json')
assert h['parse_xssi'](b")]}'\n{\"a\":1}") == {'a': 1}
assert h['parse_xssi'](b'[]') == []
with h['locked']():
    h['put']('secret:demo', 'fixture-only-value')
    assert h['get']('secret:demo') == 'fixture-only-value'
    assert 'fixture-only-value' not in (root/'vault.json').read_text()
    assert h['dispatch']('secret-has', ['demo']) == {'exists': True}
    h['dispatch']('secret-delete', ['demo'])
    assert h['get']('secret:demo') is None
    raw = json.dumps({'token':'old-fixture'})
    h['set_slots'](raw, None)
    h['dispatch']('begin-add', [])
    assert h['active']() is None
    try:
        h['dispatch']('begin-add', [])
    except h['Failure']:
        pass
    else:
        raise AssertionError('overwrote pending journal')
    h['set_slots']('new-fixture', None)
    h['dispatch']('rollback-add', [])
    assert h['active']() == raw
from contextlib import contextmanager
import io
locks = []
file_lock = h['locked']
@contextmanager
def tracked_lock():
    assert not locks
    with file_lock():
        locks.append(True)
        try:
            yield
        finally:
            locks.pop()
def contact_google(req, **kwargs):
    assert not locks, 'network request holds the vault lock'
    return io.BytesIO(b'{"assignments":[]}')
g.update(locked=tracked_lock, urlopen=contact_google)
assert h['dispatch']('assignments', ['active']) == []
g['request'] = lambda *_args, **_kwargs: {'currentBalance': 300, 'consumptionRateHourly': 2, 'assignmentsCount': 1}
assert h['dispatch']('usage', ['fixture'])['planTier'] == 'PRO+'
g['request'] = lambda *_args, **_kwargs: {'assignments': [{'endpoint':'vm', 'accelerator':'T4', 'machineShape':0, 'variant':1, 'runtimeProxyInfo':{'token':'fixture'}}]}
assert 'runtimeProxyInfo' not in h['dispatch']('assignments', [])[0]
calls = []
def release_request(account, path, xsrf=None):
    calls.append((account, path, xsrf))
    return {'token': 'fixture-xsrf'}
g['request'] = release_request
h['dispatch']('unassign', ['endpoint', 'fixture-account'])
assert calls == [('fixture-account','unassign/endpoint',None),
                 ('fixture-account','unassign/endpoint','fixture-xsrf')]
"#;
        let out = command(py)
            .args(["-B", "-c", script])
            .arg(helper)
            .arg(root.path())
            .output()
            .await
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[tokio::test]
    async fn watchdog_reloads_policy_and_protects_a_render() {
        let py = python()
            .await
            .expect("Python 3 required for watchdog tests");
        let root = tempfile::tempdir().unwrap();
        let asset = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/watchdog.py");
        let test = r#"
import io, json, runpy, subprocess, sys, types
from pathlib import Path
sys.modules['fcntl'] = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, flock=lambda *args: None)
h = runpy.run_path(sys.argv[1])
g = h['main'].__globals__
g['urlopen'] = lambda *a, **k: io.BytesIO(b'{"queue_running":[],"queue_pending":[1]}')
assert h['render_busy']()
g['urlopen'] = lambda *a, **k: io.BytesIO(b'{"queue_running":[],"queue_pending":[]}')
assert not h['render_busy']()
g['urlopen'] = lambda *a, **k: io.BytesIO(b'{}')
assert h['render_busy']()
root = Path(sys.argv[2])
cfg = root / 'watchdog.json'
def policy(deadline, idle):
    cfg.write_text(json.dumps(dict(deadline_epoch=deadline, idle_timeout_s=idle, backup_command='fixture')))
policy(95, 0)
clock = [100]
events = []
def advance(seconds):
    assert seconds == 15 and clock[0] < 130
    clock[0] += seconds
    policy(200, 30)  # extend absolute deadline and arm idle protection mid-run
def release(req, **kwargs):
    events.append((req.get_method(), clock[0]))
    assert req.full_url == 'http://fixture/unassign'
    return io.BytesIO()
g.update(ROOT=root, time=types.SimpleNamespace(time=lambda:clock[0], monotonic=lambda:clock[0], sleep=advance),
         compute_busy=lambda:False, render_busy=lambda port:clock[0]==100, runtime_address=lambda:'fixture',
         subprocess=types.SimpleNamespace(run=lambda *a, **k:events.append(('backup',clock[0]))), urlopen=release)
h['main']()
assert events == [('backup',130), ('POST',130)], events
events.clear()
policy(95, 0)
def retry(seconds):
    assert clock[0] < 160
    clock[0] += seconds
def unavailable(*args, **kwargs):
    events.append(('backup', clock[0]))
    raise OSError('fixture Drive outage')
g['time'].sleep = retry
g['subprocess'] = types.SimpleNamespace(run=unavailable, SubprocessError=subprocess.SubprocessError)
h['main']()
assert events == [('backup',130), ('backup',145), ('backup',160), ('POST',160)], events
"#;
        let out = command(py)
            .args(["-B", "-c", test])
            .arg(asset)
            .arg(root.path())
            .output()
            .await
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
