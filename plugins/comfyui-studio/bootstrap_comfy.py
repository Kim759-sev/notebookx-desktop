"""NotebookX Colab bootstrap. Progress records are line-delimited JSON."""
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import filecmp
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
from urllib.parse import quote, urlsplit
from urllib.error import HTTPError
from urllib.request import Request, urlopen


CONTENT = Path("/content")
COMFY = CONTENT / "ComfyUI"
CATEGORIES = ("checkpoints", "diffusion_models", "loras", "vae", "text_encoders", "clip",
              "clip_vision", "controlnet", "upscale_models", "semantic_bridge", "other")
MODEL_EXTENSIONS = {".safetensors", ".ckpt", ".pt", ".pth", ".gguf", ".onnx", ".bin", ".sft"}
CLOUDFLARE = re.compile(r"https://[a-zA-Z0-9-]+\.trycloudflare\.com\b")


def emit(prefix, **fields):
    print(prefix + " " + json.dumps(fields, ensure_ascii=False), flush=True)


def stage(name, status, detail, **fields):
    emit("__NBX_STAGE__", stage=name, status=status, detail=detail, **fields)


def inside(root, relative):
    path = Path(relative)
    if not relative or path.is_absolute() or ".." in path.parts or "\\" in relative:
        raise ValueError(f"Expected a relative path: {relative!r}")
    result = (root / path).resolve()
    if not result.is_relative_to(root.resolve()):
        raise ValueError(f"Path escapes configured root: {relative!r}")
    return result


def drive_root():
    root = Path(os.getenv("NBX_DRIVE_ROOT", "/content/drive/MyDrive/DramaSmith-H3")).resolve()
    # An ordinary /content/drive directory is ephemeral when the mount failed.
    mounts = Path("/proc/self/mountinfo").read_text().splitlines()
    for line in mounts:
        mount, fs = line.split(" - ", 1)
        path = Path(re.sub(r"\\([0-7]{3})", lambda m: chr(int(m[1], 8)), mount.split()[4]))
        if root.is_relative_to(path) and (path == CONTENT / "drive" or fs.startswith("fuse")):
            root.mkdir(parents=True, exist_ok=True)
            return root
    raise RuntimeError(f"Mount Google Drive before bootstrapping {root}")


def layout(root):
    for relative in [*(f"models/{c}" for c in CATEGORIES), "loras", "outputs", "workflows", "env-snapshot", "runtime-backup"]:
        inside(root, relative).mkdir(parents=True, exist_ok=True)


@contextmanager
def atomic_path(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(dir=path.parent, prefix="." + path.name + ".", suffix=".part")
    os.close(fd)
    temporary = Path(name)
    try:
        yield temporary
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def manifest(root):
    items = []
    for base in (root / "models", root / "loras"):
        for path in sorted(base.rglob("*")):
            if path.suffix.lower() not in MODEL_EXTENSIONS or not path.is_file():
                continue
            relative = path.relative_to(root)
            category = "loras" if base.name == "loras" else relative.parts[1]
            stat = path.stat()
            items.append(dict(filename=path.name, relativePath=relative.as_posix(),
                              category=category if category in CATEGORIES else "other",
                              sizeBytes=stat.st_size, updatedAt=datetime.fromtimestamp(stat.st_mtime, timezone.utc).isoformat()))
    with atomic_path(root / "models-manifest.json") as tmp:
        tmp.write_text(json.dumps(items, ensure_ascii=False), encoding="utf-8")
    stage("models", "completed", f"Indexed {len(items)} Drive files")


def request_for(url):
    parsed = urlsplit(url)
    if parsed.scheme not in ("http", "https") or not parsed.hostname or parsed.username or parsed.password:
        raise ValueError("Expected an HTTP(S) download URL")
    req = Request(url, headers={"User-Agent": "NotebookX/1.0"})
    tokens = {"huggingface.co": "HF_TOKEN", "hf.co": "HF_TOKEN", "civitai.com": "CIVITAI_TOKEN", "civitai.red": "CIVITAI_TOKEN"}
    for host, var in tokens.items():
        if parsed.scheme == "https" and (parsed.hostname == host or parsed.hostname.endswith("." + host)) and os.getenv(var):
            req.add_unredirected_header("Authorization", "Bearer " + os.environ[var])
            break
    return req


def download_one(root, item):
    filename = item["filename"]
    if not filename or Path(filename).name != filename or filename in (".", "..") or "\\" in filename:
        raise ValueError("Invalid download filename")
    category = item.get("category", "other")
    if category not in CATEGORIES:
        category = "other"
    dest = inside(root, str(PurePosixPath(item.get("targetDriveDir") or f"models/{category}") / filename))
    dest.parent.mkdir(parents=True, exist_ok=True)

    def progress(status, size, total=None, **fields):
        percent = 100 if status == "completed" else min(99, int(size * 100 / total)) if total else 0
        emit("__NBX_DL__", id=item.get("id"), filename=filename, status=status, downloadedBytes=size,
             totalBytes=total, progressPercent=percent, **fields)

    req = request_for(item["sourceUrl"])
    if dest.is_file() and dest.stat().st_size > 0:
        existing = dest.stat().st_size
        if item.get("totalBytes") and existing != item["totalBytes"]:
            raise ValueError(f"Existing {filename} differs from the queued size")
        progress("completed", existing, existing)
        return
    progress("downloading", 0)
    with atomic_path(dest) as tmp:
        with urlopen(req, timeout=120) as stream:
            url = stream.geturl()
            length = stream.headers.get("Content-Length")
            total = int(length) if length else item.get("totalBytes")
            # Resolve signed CDN URLs with urllib so tokens never enter subprocess
            # arguments or follow a cross-origin redirect as an aria2 custom header.
            if shutil.which("aria2c") and not (req.has_header("Authorization") and url == req.full_url):
                stream.close()
                command = ["aria2c", "-x", "16", "-s", "16", "--no-conf", "--file-allocation=none",
                           "--allow-overwrite=true", "--auto-file-renaming=false", "--summary-interval=1",
                           "--show-console-readout=true", "--human-readable=false", "--enable-color=false",
                           "--download-result=hide", "-d", str(dest.parent), "-o", tmp.name, "--", url]
                try:
                    with subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace") as process:
                        for line in process.stdout:
                            match = re.search(r"\[#\w+\s+(\d+)B?/(\d+)B?", line)
                            if match:
                                progress("downloading", int(match[1]), int(match[2]) or total)
                        if process.wait():
                            raise RuntimeError(f"aria2c failed for {filename} (exit {process.returncode})")
                finally:
                    Path(str(tmp) + ".aria2").unlink(missing_ok=True)
            else:
                received = 0
                last_update = 0
                with tmp.open("wb") as out:
                    while block := stream.read(1024 ** 2):
                        out.write(block)
                        received += len(block)
                        if time.monotonic() - last_update >= 1:
                            progress("downloading", received, total)
                            last_update = time.monotonic()
        size = tmp.stat().st_size
        if not size or (total is not None and size != total):
            raise IOError(f"Incomplete download of {filename}")
    progress("completed", size, size)


def downloads(root):
    queue = json.loads(os.getenv("NBX_DOWNLOAD_QUEUE_JSON") or "[]")
    if not isinstance(queue, list):
        raise ValueError("NBX_DOWNLOAD_QUEUE_JSON must contain an array")
    failures = []
    for item in queue:
        try:
            download_one(root, item)
        except Exception as exc:
            name = item.get("filename", "?") if isinstance(item, dict) else "?"
            # Error text from network clients can include a signed URL.
            detail = re.sub(r"https?://\S+", "[download URL]", str(exc))
            emit("__NBX_DL__", id=item.get("id") if isinstance(item, dict) else None, filename=name, status="failed", error=detail)
            failures.append(name)
    manifest(root)
    if failures:
        raise RuntimeError("Downloads failed: " + ", ".join(failures))


def copy_tree(source, dest, *, missing_only=False, keep_conflicts=False):
    if source.resolve() == dest.resolve() or not source.exists():
        return
    if source.is_dir():
        dest.mkdir(parents=True, exist_ok=True)
        for child in source.iterdir():
            if not child.is_symlink():
                copy_tree(child, dest / child.name, missing_only=missing_only, keep_conflicts=keep_conflicts)
    else:
        if dest.exists():
            if keep_conflicts:
                if filecmp.cmp(source, dest, shallow=False):
                    return
                dest = dest.with_name(f"{dest.stem}.recovered-{time.time_ns()}{dest.suffix}")
            elif missing_only or (dest.stat().st_size == source.stat().st_size and dest.stat().st_mtime_ns == source.stat().st_mtime_ns):
                return
        with atomic_path(dest) as tmp:
            shutil.copy2(source, tmp)


def bind_dir(local, persistent):
    persistent.mkdir(parents=True, exist_ok=True)
    local.parent.mkdir(parents=True, exist_ok=True)
    if local.resolve() == persistent.resolve():
        return
    if local.is_symlink():
        raise RuntimeError(f"{local} already points to {local.resolve()}")
    if local.exists():
        # Retain files from an existing local install, even if names clash.
        copy_tree(local, persistent, keep_conflicts=True)
        local.rename(local.with_name(local.name + ".before-drive-" + str(time.time_ns())))
    local.symlink_to(persistent, target_is_directory=True)


def configure(root):
    categories = {c: f"models/{c}" for c in CATEGORIES}
    categories["loras"] = "models/loras\nloras"
    # JSON is valid YAML and quotes all paths, including names with punctuation.
    (COMFY / "extra_model_paths.yaml").write_text(json.dumps({"drive": {"base_path": str(root), **categories}}, ensure_ascii=False), encoding="utf-8")
    bind_dir(COMFY / "output", root / "outputs")
    bind_dir(COMFY / "user/default/workflows", root / "workflows")
    settings = COMFY / "user/default/comfy.settings.json"
    saved = root / "runtime-backup/user/default/comfy.settings.json"
    if settings.resolve() != saved.resolve():
        saved.parent.mkdir(parents=True, exist_ok=True)
        if not saved.exists():
            legacy = root / "runtime-backup/comfy.settings.json"
            if legacy.is_file() or settings.is_file():
                copy_tree(legacy if legacy.is_file() else settings, saved)
            else:
                saved.write_text("{}", encoding="utf-8")
        if settings.exists() or settings.is_symlink():
            settings.rename(settings.with_name(settings.name + ".before-drive-" + str(time.time_ns())))
        settings.symlink_to(saved)
    for relative in ("input", "user/__h3c_api"):
        copy_tree(root / "runtime-backup" / relative, COMFY / relative, missing_only=True)


def backup(root):
    if not COMFY.is_dir():
        stage("backup", "skipped", "No local ComfyUI installation")
        return
    for local, remote in [("output", "outputs"), ("h3_sequences", "outputs/h3_sequences"),
                          ("input", "runtime-backup/input"), ("user/__h3c_api", "runtime-backup/user/__h3c_api"),
                          ("user/default/workflows", "workflows"), ("user/default/comfy.settings.json", "runtime-backup/user/default/comfy.settings.json")]:
        copy_tree(COMFY / local, root / remote)
    os.sync()
    stage("backup", "completed", "Outputs, sequences, inputs and settings flushed to Drive")


def save_snapshot(root):
    if not (COMFY / "main.py").is_file():
        raise RuntimeError("No ComfyUI installation to snapshot")
    excluded = {"models", "output", "input", "temp", "venv", ".venv", ".git", "__pycache__", "h3_sequences"}

    def keep(member):
        parts = PurePosixPath(member.name).parts
        if member.issym() or (len(parts) > 1 and parts[1] in excluded) or (len(parts) > 2 and parts[1] == "user" and parts[2] != "default") or any(p in {".git", "__pycache__"} or ".before-drive-" in p for p in parts):
            return None
        return member

    with atomic_path(root / "env-snapshot/comfyui_env.tar.gz") as tmp:
        with tarfile.open(tmp, "w:gz", compresslevel=1, dereference=False) as archive:
            archive.add(COMFY, arcname="ComfyUI", filter=keep)
            # Drive-backed custom node directories commonly use top-level symlinks.
            custom = COMFY / "custom_nodes"
            if custom.is_dir():
                for node in custom.iterdir():
                    if node.is_dir() and (node.is_symlink() or custom.is_symlink()):
                        archive.add(node.resolve(), arcname=f"ComfyUI/custom_nodes/{node.name}", filter=keep)
            settings = COMFY / "user/default/comfy.settings.json"
            if settings.is_symlink() and settings.is_file():
                archive.add(settings.resolve(), arcname="ComfyUI/user/default/comfy.settings.json")
    stage("snapshot", "completed", "Saved ComfyUI core, custom nodes and user defaults")


def restore(snapshot):
    # Stage before merging so a truncated archive cannot partly replace the install.
    with tempfile.TemporaryDirectory(dir=CONTENT) as temporary:
        directory = Path(temporary)
        decoder = None
        try:
            if snapshot.suffix == ".zst":
                if shutil.which("zstd"):
                    command = ["zstd", "-dc", snapshot]
                else:
                    # Colab images differ in preinstalled utilities. Keep the
                    # decompressor streaming on machines without the zstd CLI.
                    try:
                        import zstandard
                    except ImportError:
                        subprocess.run([sys.executable, "-m", "pip", "install", "zstandard"], check=True)
                    command = [sys.executable, "-c",
                               "import sys,zstandard as z; z.ZstdDecompressor().copy_stream(open(sys.argv[1],'rb'),sys.stdout.buffer)", snapshot]
                decoder = subprocess.Popen(command, stdout=subprocess.PIPE)
                archive = tarfile.open(fileobj=decoder.stdout, mode="r|")
            else:
                archive = tarfile.open(snapshot, "r:gz")
            with archive:
                for member in archive:
                    path = PurePosixPath(member.name)
                    if path.is_absolute() or ".." in path.parts:
                        raise ValueError("Invalid snapshot path")
                    if not (member.isdir() or member.isfile()):
                        continue
                    parts = path.parts
                    if parts[:2] == ("content", "ComfyUI"):
                        parts = parts[2:]
                    elif parts[:1] == ("ComfyUI",):
                        parts = parts[1:]
                    if not parts:
                        continue
                    dest = directory.joinpath(*parts)
                    if member.isdir():
                        dest.mkdir(parents=True, exist_ok=True)
                    else:
                        dest.parent.mkdir(parents=True, exist_ok=True)
                        with archive.extractfile(member) as src, dest.open("wb") as out:
                            shutil.copyfileobj(src, out)
                        dest.chmod(member.mode & 0o777)
            if decoder:
                # Drain tar padding before waiting for a streaming zstd decoder.
                while decoder.stdout.read(65536):
                    pass
                if decoder.wait():
                    raise RuntimeError("zstd could not decompress the snapshot")
        finally:
            if decoder:
                decoder.stdout.close()
                if decoder.poll() is None:
                    decoder.kill()
                decoder.wait()
        ensure_core(directory if (directory / "main.py").is_file() else COMFY)
        if not COMFY.exists():
            directory.rename(COMFY)
        else:
            copy_tree(directory, COMFY, missing_only=True)


def ensure_core(location):
    if (location / "main.py").is_file():
        return
    with tempfile.TemporaryDirectory(dir=CONTENT) as temporary:
        clean = Path(temporary) / "ComfyUI"
        subprocess.run(["git", "clone", "--depth", "1", "https://github.com/comfyanonymous/ComfyUI.git", clean], check=True)
        if location.exists():
            copy_tree(clean, location, missing_only=True)
        else:
            clean.rename(location)


def install():
    manager = COMFY / "custom_nodes/ComfyUI-Manager"
    if not manager.exists():
        subprocess.run(["git", "clone", "--depth", "1", "https://github.com/Comfy-Org/ComfyUI-Manager.git", manager], check=True)
    dependencies = [COMFY / "requirements.txt", *sorted((COMFY / "custom_nodes").glob("*/requirements.txt"))]
    for path in dependencies:
        if not path.is_file():
            continue
        if shutil.which("uv"):
            result = subprocess.run(["uv", "pip", "install", "--system", "--python", sys.executable, "-r", path], cwd=path.parent)
            if result.returncode == 0:
                continue
        subprocess.run([sys.executable, "-m", "pip", "install", "-r", path], cwd=path.parent, check=True)


def json_api(port, path):
    with urlopen(f"http://127.0.0.1:{port}{path}", timeout=4) as response:
        return json.load(response)


def listening(port):
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=1):
            return True
    except OSError:
        return False


def required_nodes():
    raw = os.getenv("NBX_REQUIRED_NODES", "").strip()
    nodes = json.loads(raw) if raw.startswith("[") else [n.strip() for n in raw.split(",") if n.strip()]
    if not isinstance(nodes, list) or not all(isinstance(n, str) for n in nodes):
        raise ValueError("NBX_REQUIRED_NODES must name node classes")
    return nodes


def monitor(root, port, management, proc, log, started, *, degraded=False):
    deadline = time.monotonic() + int(os.getenv("NBX_STARTUP_TIMEOUT_SECONDS", "1800"))
    offset = 0
    tail = ""
    tunnel = None

    def read_tunnel():
        nonlocal offset, tail, tunnel
        if tunnel:
            return
        marker = root / "latest_tunnel_url.txt"
        text = ""
        if marker.is_file() and marker.stat().st_mtime >= started:
            text = marker.read_text(errors="replace")[:1024]
        if log.is_file():
            with log.open("rb") as stream:
                stream.seek(offset)
                chunk = stream.read(1024 * 1024).decode(errors="replace")
                offset = stream.tell()
            text += "\n" + tail + chunk
            tail = chunk[-100:]
        if match := CLOUDFLARE.search(text):
            tunnel = match[0]
            stage("cloudflare", "completed", tunnel, cloudflareUrl=tunnel)

    while time.monotonic() < deadline:
        read_tunnel()
        if proc and proc.poll() not in (None, 0):
            raise RuntimeError(f"Startup exited with code {proc.returncode}; see {log}")
        try:
            if (not management or listening(management)) and isinstance(json_api(port, "/system_stats"), dict):
                queue = json_api(port, "/queue")
                if "queue_running" in queue and "queue_pending" in queue:
                    break
        except (OSError, ValueError):
            pass
        time.sleep(2)
    else:
        raise TimeoutError(f"ComfyUI services did not become ready; see {log}")
    read_tunnel()
    loaded, missing = [], []
    for node in required_nodes():
        try:
            data = json_api(port, "/object_info/" + quote(node, safe=""))
        except HTTPError as exc:
            if exc.code != 404:
                raise
            data = {}
        (loaded if node in data else missing).append(node)
    if missing and not degraded:
        stage("nodes", "failed", ", ".join(missing), loadedRequiredNodes=loaded, missingRequiredNodes=missing)
        raise RuntimeError("Missing required nodes: " + ", ".join(missing))
    stage("nodes", "skipped" if missing else "completed", ", ".join(missing or loaded),
          loadedRequiredNodes=loaded, missingRequiredNodes=missing)
    stage("ready", "completed", f"ComfyUI ready on {port}", comfyPort=port, managementPort=management or None,
          cloudflareUrl=tunnel, loadedRequiredNodes=loaded, missingRequiredNodes=missing)


def start(root, mode):
    port = int(os.getenv("NBX_COMFY_PORT") or 8188)
    management = int(os.getenv("NBX_MGMT_PORT", "8189" if mode == "managedScript" else "0") or 0)
    script = inside(root, os.getenv("NBX_MANAGED_SCRIPT") or "workflows/startup-managed-v2.py")
    managed = mode == "managedScript" and script.is_file()
    fallback = mode == "managedScript" and not managed
    if managed and not all((root / name).is_dir() for name in ("models", "workflows")):
        raise RuntimeError("Managed Drive is missing its models or workflows directory")
    layout(root)
    print("DRIVE_MOUNT_OK True", flush=True)
    stage("drive", "completed", str(root))
    if mode == "cpuPreDownload":
        downloads(root)
        stage("ready", "completed", "CPU download queue finished")
        return
    if mode not in ("managedScript", "comfySnapshot"):
        raise ValueError("Unknown NBX_STARTUP_MODE")
    if fallback:
        stage("managedScript", "skipped", "Managed script absent; starting a standard ComfyUI snapshot")
    if not managed:
        management = 0
    log = CONTENT / ("nbx-managed.log" if managed else "nbx-comfy.log")
    proc = None
    started = time.time()
    if listening(port) or (management and listening(management)):
        stage("launch", "running", "Checking existing services")
    else:
        if managed:
            command, cwd = [sys.executable, "-u", str(script)], CONTENT
        else:
            snapshots = [p for ext in ("zst", "gz") if (p := root / f"env-snapshot/comfyui_env.tar.{ext}").is_file()]
            if snapshots:
                newest = max(snapshots, key=lambda p: p.stat().st_mtime_ns)
                stage("snapshot", "running", f"Restoring {newest.name}")
                restore(newest)
            else:
                stage("snapshot", "running", "Installing ComfyUI on local disk")
                ensure_core(COMFY)
            install()
            configure(root)
            stage("snapshot", "completed", "Local environment and Drive paths ready")
            command, cwd = [sys.executable, "-u", "main.py", "--listen", "127.0.0.1", "--port", str(port)], COMFY
        stage("launch", "running", str(log))
        with log.open("wb") as stream:
            proc = subprocess.Popen(command, cwd=cwd, stdout=stream, stderr=subprocess.STDOUT,
                                    stdin=subprocess.DEVNULL, start_new_session=True)
    manifest(root)
    monitor(root, port, management, proc, log, started, degraded=fallback)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--save-snapshot", action="store_true")
    group.add_argument("--pre-stop-backup", action="store_true")
    # Colab CLI executes this source in an IPython kernel that has its own argv.
    args = parser.parse_args([] if "get_ipython" in globals() else None)
    root = drive_root()
    mode = os.getenv("NBX_STARTUP_MODE", "managedScript")
    if args.save_snapshot or mode == "saveSnapshot":
        save_snapshot(root)
    elif args.pre_stop_backup or mode == "preStopBackup":
        backup(root)
    else:
        start(root, mode)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        stage("bootstrap", "failed", str(exc))
        sys.exit(1)
