"""Colab CUDA worker. Set NBX_HF_REPO to owner/repo and optionally HF_TOKEN."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote
from urllib.request import Request, urlopen


MIB = 1024 ** 2
QUANT = re.compile(r"(?<![a-z0-9])((?:UD[-_])?(?:I?Q[1-8](?:_[A-Z0-9]+)*|BF16|F16|FP16|F32|FP32))(?![a-z0-9])", re.I)
SHARD = re.compile(r"-(\d+)-of-(\d+)$", re.I)
CONTEXTS = (32768, 16384, 8192)


def stage(name, status, detail):
    print("__NBX_STAGE__ " + json.dumps(dict(stage=name, status=status, detail=detail)), flush=True)


def hub(url, token):
    request = Request(url, headers={"User-Agent": "NotebookX/1.0"})
    # urllib copies ordinary headers across redirects (including to storage CDNs).
    if token:
        request.add_unredirected_header("Authorization", "Bearer " + token)
    return urlopen(request, timeout=120)


def precision(label):
    if label in {"F32", "FP32", "BF16", "FP16", "F16"}:
        return (0, 32 if "32" in label else 16, 0)
    bits = int(re.search(r"Q(\d)", label).group(1))
    suffix = label.split("_")[-1]
    grade = {"XXS": 0, "XS": 1, "S": 2, "0": 3, "K": 4, "M": 5, "1": 5, "L": 6, "XL": 7}.get(suffix, 3)
    return (1, bits, grade)


def variants(metadata):
    groups = {}
    projectors = []
    for file in metadata.get("siblings", []):
        path = file["rfilename"]
        if not path.lower().endswith(".gguf"):
            continue
        if PurePosixPath(path).is_absolute() or ".." in PurePosixPath(path).parts or "\\" in path:
            raise ValueError("Invalid path in Hub listing")
        size = file.get("size") or (file.get("lfs") or {}).get("size") or 0
        stem = path[:-5]
        if "mmproj" in stem.lower():
            projectors.append((path, size))
            continue
        shard = SHARD.search(stem)
        family = stem[:shard.start()] if shard else stem
        labels = QUANT.findall(family)
        if labels:
            label = labels[-1].upper().replace("UD_", "UD-")
            index, count = map(int, shard.groups()) if shard else (1, 1)
            groups.setdefault((label, family), []).append((index, count, path, size))
    result = []
    for (label, family), shards in sorted(groups.items()):
        shards.sort()
        count = shards[0][1]
        if len(shards) != count or any(i != expected or n != count or size <= 0 for expected, (i, n, _, size) in enumerate(shards, 1)):
            continue
        # Prefer a projector in the same subdirectory, then the smallest compatible candidate.
        proj = min(projectors, key=lambda p: (PurePosixPath(p[0]).parent != PurePosixPath(family).parent, p[1], p[0]), default=None)
        if proj and not proj[1]:
            continue
        files = [(path, size) for _, _, path, size in shards]
        result.append((label, files, proj))
    return sorted(result, key=lambda v: precision(v[0]), reverse=True)


def fits(files, projector, ctx, budget):
    weight_bytes = sum(size for _, size in files) + (projector[1] if projector else 0)
    # Q8 K/V allowance of 256 KiB per token plus CUDA compute/graph buffers.
    estimate = math.ceil(weight_bytes / MIB) + ctx // 4 + 768
    return estimate * 100 <= budget * 95


def fetch_file(repo, revision, file, cache, token):
    name, size = file
    path = cache / name
    if path.is_file() and path.stat().st_size == size:
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as tmp:
        partial = Path(tmp.name)
    try:
        with hub(f"https://huggingface.co/{repo}/resolve/{quote(revision, safe='')}/{quote(name)}", token) as stream, partial.open("wb") as dest:
            shutil.copyfileobj(stream, dest, 4 * MIB)
        if partial.stat().st_size != size:
            raise RuntimeError(f"Incomplete GGUF download: {name}")
        partial.replace(path)
    finally:
        partial.unlink(missing_ok=True)
    return path


def has_cuda(binary):
    result = subprocess.run([binary, "--list-devices"], capture_output=True, text=True)
    return result.returncode == 0 and re.search(r"CUDA\d+\s*:", result.stdout + result.stderr) is not None


def server_binary(root):
    source = root / "llama.cpp"
    built = source / "build/bin/llama-server"
    for binary in [shutil.which("llama-server"), built]:
        if binary and Path(binary).is_file() and has_cuda(binary):
            return binary
    if not (source / "CMakeLists.txt").is_file():
        subprocess.run(["git", "clone", "--depth", "1", "https://github.com/ggml-org/llama.cpp.git", source], check=True)
    subprocess.run(["cmake", "-S", source, "-B", source / "build", "-DGGML_CUDA=ON", "-DCMAKE_CUDA_ARCHITECTURES=native",
                    "-DCMAKE_BUILD_TYPE=Release", "-DLLAMA_BUILD_SERVER=ON", "-DLLAMA_BUILD_UI=ON"], check=True)
    subprocess.run(["cmake", "--build", source / "build", "--config", "Release", "--target", "llama-server", "-j", str(min(4, os.cpu_count() or 1))], check=True)
    if not has_cuda(built):
        raise RuntimeError("llama-server has no visible CUDA device")
    return built


def serve(args):
    if not re.fullmatch(r"[A-Za-z0-9_-][\w.-]*/[A-Za-z0-9_-][\w.-]*", args.repo) or not 0 < args.port < 65536:
        raise ValueError("Provide owner/repo and a valid TCP port")
    root = Path("/content/notebookx-llama")
    root.mkdir(parents=True, exist_ok=True)
    try:
        with socket.create_connection(("127.0.0.1", args.port), timeout=1):
            raise ValueError(f"Port {args.port} is already in use")
    except ConnectionRefusedError:
        pass
    token = os.getenv("HF_TOKEN") or os.getenv("PLUGIN_HF_TOKEN")
    with hub(f"https://huggingface.co/api/models/{args.repo}?blobs=true", token) as response:
        metadata = json.load(response)
    revision = metadata["sha"]
    cache = root / "models" / hashlib.sha256(f"{args.repo}@{revision}".encode()).hexdigest()
    # Use the first CUDA device consistently with llama-server. Total memory alone
    # would count memory already occupied by another process as available.
    gpu = subprocess.check_output(["nvidia-smi", "--query-gpu=memory.total,memory.free", "--format=csv,noheader,nounits", "-i", "0"], text=True)
    total, free = map(int, gpu.strip().split(","))
    budget = min(total, free)
    stage("gguf-inspection", "completed", f"{free}/{total} MiB available")
    trained = (metadata.get("gguf") or {}).get("context_length") or CONTEXTS[0]
    contexts = [ctx for ctx in CONTEXTS if ctx <= max(trained, CONTEXTS[-1])]
    options = [(label, files, proj, ctx) for label, files, proj in variants(metadata) for ctx in contexts
               if (args.quant.lower() == "auto" or args.quant.upper() == label) and fits(files, proj, ctx, budget)]
    if not options:
        raise RuntimeError("No complete GGUF variant fits the 95% VRAM budget at 8192 tokens")
    stage("llama-server", "running", "Preparing CUDA llama-server")
    binary = server_binary(root)
    log = root / "llama-server.log"
    for label, files, projector, ctx in options:
        stage("gguf-download", "running", f"{label}, {ctx} tokens")
        weights = [fetch_file(args.repo, revision, file, cache, token) for file in files]
        command = [binary, "-m", weights[0], "--host", "127.0.0.1", "--port", str(args.port), "--ctx-size", str(ctx), "--parallel", "1",
                   "--n-gpu-layers", "999", "--cache-type-k", "q8_0", "--cache-type-v", "q8_0", "--flash-attn", "on", "--alias", args.repo]
        if projector:
            command.extend(["--mmproj", fetch_file(args.repo, revision, projector, cache, token)])
        with log.open("wb") as output:
            proc = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True,
                                    env={**os.environ, "CUDA_VISIBLE_DEVICES": "0"})
        deadline = time.monotonic() + 600
        over_budget = False
        try:
            while proc.poll() is None and time.monotonic() < deadline:
                try:
                    with urlopen(f"http://127.0.0.1:{args.port}/health", timeout=2) as response:
                        if json.load(response).get("status") == "ok":
                            now_free = int(subprocess.check_output(["nvidia-smi", "--query-gpu=memory.free", "--format=csv,noheader,nounits", "-i", "0"], text=True).strip())
                            if (free - now_free) * 100 > budget * 95:
                                over_budget = True
                                break
                            with urlopen(f"http://127.0.0.1:{args.port}/", timeout=4) as page:
                                if "text/html" not in page.headers.get("Content-Type", ""):
                                    raise RuntimeError("llama-server did not serve its Web Chat UI")
                            print("__NBX_LLAMA__ " + json.dumps(dict(chosenQuant=label, ctxSize=ctx, port=args.port)), flush=True)
                            stage("llama-server", "completed", f"http://127.0.0.1:{args.port}")
                            return
                except (OSError, ValueError):
                    pass
                time.sleep(1)
        except BaseException:
            proc.terminate()
            raise
        if proc.poll() is None:
            proc.terminate()
            proc.wait(timeout=30)
            if over_budget:
                stage("llama-server", "running", f"{label} at {ctx} exceeded measured VRAM budget; trying a smaller configuration")
                continue
            raise TimeoutError(f"llama-server startup timed out; see {log}")
        with log.open("rb") as output:
            output.seek(max(0, log.stat().st_size - 8192))
            detail = output.read().decode(errors="replace")
        if not re.search(r"out of memory|cudaMalloc|failed to allocate|unable to allocate", detail, re.I):
            raise RuntimeError(f"llama-server startup failed: {detail[-1500:]}")
        stage("llama-server", "running", f"Allocation failed for {label} at {ctx}; retrying a smaller configuration")
    raise RuntimeError("CUDA could not allocate any candidate at 8192 tokens or more")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=os.getenv("NBX_HF_REPO") or os.getenv("PLUGIN_INPUT_REPO", ""))
    parser.add_argument("--quant", default=os.getenv("NBX_LLAMA_QUANT") or os.getenv("PLUGIN_INPUT_QUANT", "auto"))
    parser.add_argument("--port", type=int, default=os.getenv("NBX_LLAMA_PORT") or os.getenv("PLUGIN_INPUT_PORT", "8080"))
    try:
        serve(parser.parse_args([] if "get_ipython" in globals() else None))
    except Exception as exc:
        stage("llama-server", "failed", str(exc))
        sys.exit(1)
