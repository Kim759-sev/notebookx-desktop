"""Detached VM deadline and idle monitor. The config is atomically replaceable."""
import errno
import fcntl
import json
import os
from pathlib import Path
import subprocess
import time
from urllib.error import URLError
from urllib.request import Request, urlopen

ROOT = Path("/root/.notebookx")
POLL_SECONDS = 15


def render_busy(port=8188):
    try:
        with urlopen(f"http://127.0.0.1:{port}/queue", timeout=4) as response:
            queue = json.load(response)
        if not all(isinstance(queue[k], list) for k in ("queue_running", "queue_pending")):
            return True
        return bool(queue["queue_running"] or queue["queue_pending"])
    except URLError as exc:
        # CPU/download sessions and sessions still bootstrapping have no server.
        return getattr(exc.reason, "errno", None) != errno.ECONNREFUSED
    except (OSError, ValueError, KeyError, TypeError):
        return True


def compute_busy():
    try:
        if time.time() - (ROOT / "heartbeat").stat().st_mtime < 2 * POLL_SECONDS:
            return True
    except FileNotFoundError:
        pass
    try:
        result = subprocess.run(
            ["nvidia-smi", "--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=5, check=True)
        return any(float(line) >= 5 for line in result.stdout.splitlines() if line.strip())
    except (OSError, subprocess.SubprocessError, ValueError):
        return False


def runtime_address():
    if os.environ.get("TBE_RUNTIME_ADDR"):
        return os.environ["TBE_RUNTIME_ADDR"]
    for path in Path("/proc").glob("[0-9]*/environ"):
        try:
            for item in path.read_bytes().split(b"\0"):
                if item.startswith(b"TBE_RUNTIME_ADDR="):
                    return item.partition(b"=")[2].decode()
        except (OSError, UnicodeError):
            continue
    raise RuntimeError("Runtime address unavailable")


def policy_due(config, idle_elapsed):
    deadline = config.get("deadline_epoch")
    idle = config.get("idle_timeout_s")
    return bool((deadline and time.time() >= deadline) or (idle and idle_elapsed >= idle))


def read_config():
    return json.loads((ROOT / "watchdog.json").read_text())


def runtime_settings():
    path = ROOT / "runtime-settings.json"
    return json.loads(path.read_text()) if path.exists() else {}


def main():
    ROOT.mkdir(mode=0o700, parents=True, exist_ok=True)
    with (ROOT / "watchdog.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return
        last_busy = time.monotonic()
        backup_failures = 0
        while True:
            try:
                config = read_config()
                settings = runtime_settings()
                port = int(settings.get("NBX_COMFY_PORT", 8188))
                queued = render_busy(port)
                if queued or compute_busy():
                    last_busy = time.monotonic()
                due = not queued and policy_due(config, time.monotonic() - last_busy)
                if not due:
                    backup_failures = 0
                else:
                    if config.get("backup_command"):
                        try:
                            subprocess.run(config["backup_command"], shell=True, check=True,
                                           env={**os.environ, **settings},
                                           timeout=config.get("backup_timeout_s", 900))
                        except (OSError, subprocess.SubprocessError) as exc:
                            backup_failures += 1
                            print("Wind-down backup failed:", type(exc).__name__, flush=True)
                            # An unavailable Drive must not disable the cost guard forever.
                            if backup_failures < max(1, config.get("backup_attempts", 3)):
                                time.sleep(POLL_SECONDS)
                                continue
                    # A backup can take minutes. Recheck extensions and new renders.
                    queued = render_busy(int(runtime_settings().get("NBX_COMFY_PORT", 8188)))
                    if queued or compute_busy():
                        last_busy = time.monotonic()
                    if not queued and policy_due(read_config(), time.monotonic() - last_busy):
                        request = Request("http://" + runtime_address() + "/unassign", data=b"")
                        with urlopen(request, timeout=15):
                            return
            except Exception as exc:
                # Keep retrying on a failed backup, transient API outage or bad config.
                print("Watchdog cycle failed:", type(exc).__name__, flush=True)
            time.sleep(POLL_SECONDS)


if __name__ == "__main__":
    main()
