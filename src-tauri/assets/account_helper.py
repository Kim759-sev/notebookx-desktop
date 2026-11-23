"""NotebookX credential bridge. Only the marker line is part of its protocol."""
import base64
from contextlib import contextmanager, redirect_stderr, redirect_stdout
from datetime import datetime, timedelta, timezone
import hashlib
import json
import logging
import math
import os
from pathlib import Path
import secrets
import sys
import tempfile
from urllib.error import HTTPError
from urllib.parse import parse_qs, quote, urlencode, urlparse
from urllib.request import Request, urlopen

ROOT = Path.home() / ".notebookx-desktop"
VAULT = ROOT / "vault.json"
LEGACY = Path.home() / ".config/colab-cli/token.json"
SERVICE = "notebookx-desktop"
CLI_SLOT = ("google-colab-cli", "oauth_token")
TUN = "https://colab.research.google.com/tun/m/"
DEFAULT_CLIENT_ID = "764086051850-6qr4p6gpi6hn506pt8ejuq83di341hur.apps.googleusercontent.com"
DEFAULT_CLIENT_SECRET = "d-FL95Q19q7MQmFpd7hHD0Ty"
OAUTH_AUTH_URI = "https://accounts.google.com/o/oauth2/auth"
OAUTH_TOKEN_URI = "https://oauth2.googleapis.com/token"
REMOTE_REDIRECT_URI = "https://sdk.cloud.google.com/applicationdefaultauthcode.html"
PUBLIC_SCOPES = [
    "openid",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/colaboratory",
    "https://www.googleapis.com/auth/drive.file",
]


class Failure(Exception):
    """Contains only messages constructed here, safe to return to the UI."""


@contextmanager
def locked():
    ROOT.mkdir(mode=0o700, parents=True, exist_ok=True)
    with (ROOT / "vault.lock").open("a+b") as file:
        if os.name == "nt":
            import msvcrt
            file.seek(0)
            msvcrt.locking(file.fileno(), msvcrt.LK_LOCK, 1)
        else:
            import fcntl
            fcntl.flock(file, fcntl.LOCK_EX)
        # Closing the file releases the OS lock even after a crash.
        yield


def write_private(path, data):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    fd, tmp = tempfile.mkstemp(dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as file:
            file.write(data)
            file.flush()
            os.fsync(file.fileno())
        os.chmod(tmp, 0o600)
        os.replace(tmp, path)
    finally:
        Path(tmp).unlink(missing_ok=True)


def dpapi(data, decrypt=False):
    import ctypes as c
    from ctypes import wintypes

    class Blob(c.Structure):
        _fields_ = [("size", wintypes.DWORD), ("data", c.c_void_p)]

    buf = c.create_string_buffer(data)
    source = Blob(len(data), c.addressof(buf))
    out = Blob()
    api = c.windll.crypt32
    operation = api.CryptUnprotectData if decrypt else api.CryptProtectData
    if not operation(c.byref(source), None, None, None, None, 1, c.byref(out)):
        raise Failure("Windows could not unlock the local credential vault.")
    try:
        return c.string_at(out.data, out.size)
    finally:
        c.windll.kernel32.LocalFree(c.c_void_p(out.data))


def encode(value):
    data = value.encode()
    if os.name == "nt":
        return "dpapi:" + base64.b64encode(dpapi(data)).decode()
    # The CLI normally supplies cryptography through google-auth. Minimal
    # Python installations retain owner-only permissions and obscured storage.
    try:
        from cryptography.fernet import Fernet
    except ImportError:
        return "b85:" + base64.b85encode(data).decode()
    key = ROOT / "vault.key"
    if not key.exists():
        write_private(key, Fernet.generate_key().decode())
    return "fernet:" + Fernet(key.read_bytes()).encrypt(data).decode()


def decode(value):
    kind, data = value.split(":", 1)
    if kind == "dpapi":
        return dpapi(base64.b64decode(data), decrypt=True).decode()
    if kind == "b85":
        return base64.b85decode(data).decode()
    if kind == "fernet":
        from cryptography.fernet import Fernet
        return Fernet((ROOT / "vault.key").read_bytes()).decrypt(data.encode()).decode()
    raise Failure("Unsupported credential vault format.")


def keyring_call(action, service, key, *args):
    try:
        import keyring
        return True, getattr(keyring, action + "_password")(service, key, *args)
    except Exception:
        return False, None


def vault():
    return json.loads(VAULT.read_text(encoding="utf-8")) if VAULT.exists() else {}


def get(key):
    local = vault()
    if key in local:
        return decode(local[key]) if local[key] is not None else None
    if key in ("add-backup", "add-flow"):
        return None
    return keyring_call("get", SERVICE, key)[1]


def put(key, value):
    local = vault()
    # Recovery journal must remain available even if keyring availability
    # changes halfway through a sign-in.
    if key in ("add-backup", "add-flow"):
        if value is None:
            local.pop(key, None)
        else:
            local[key] = encode(value)
    elif value is None:
        keyring_call("delete", SERVICE, key)
        # A locked keyring may still contain an old entry.
        local[key] = None
    elif keyring_call("set", SERVICE, key, value)[0]:
        local.pop(key, None)
    else:
        local[key] = encode(value)
    write_private(VAULT, json.dumps(local))


def slots():
    return {"ring": keyring_call("get", *CLI_SLOT)[1],
            "file": LEGACY.read_text(encoding="utf-8") if LEGACY.exists() else None}


def set_slots(ring, file):
    if ring is None:
        keyring_call("delete", *CLI_SLOT)
        if keyring_call("get", *CLI_SLOT)[1] is not None:
            raise Failure("Unlock the active CLI credential before switching accounts.")
    elif not keyring_call("set", *CLI_SLOT, ring)[0]:
        if keyring_call("get", *CLI_SLOT)[1] not in (None, ring):
            raise Failure("Unable to replace the active CLI credential.")
        file = ring
    if file is None:
        LEGACY.unlink(missing_ok=True)
    else:
        write_private(LEGACY, file)


def active():
    pair = slots()
    return pair["ring"] or pair["file"]


def credential(account):
    raw = active() if account == "active" else get("account:" + account)
    if not raw:
        if account == "active":
            raise Failure("未检测到命令行已登录的 Colab 凭证，请点击「账号授权」在浏览器登录 Google 账号。")
        raise Failure("No credential is available for this account.")
    return json.loads(raw)


def _stdlib_refresh(info):
    body = urlencode({
        "client_id": info.get("client_id") or DEFAULT_CLIENT_ID,
        "client_secret": info.get("client_secret") or DEFAULT_CLIENT_SECRET,
        "refresh_token": info["refresh_token"],
        "grant_type": "refresh_token",
    }).encode("utf-8")
    req = Request(
        info.get("token_uri") or OAUTH_TOKEN_URI,
        data=body,
        headers={"Content-Type": "application/x-www-form-urlencoded"},
    )
    with urlopen(req, timeout=20) as resp:
        tok = json.load(resp)
    info = dict(info)
    info["token"] = tok["access_token"]
    if tok.get("id_token"):
        info["id_token"] = tok["id_token"]
    if tok.get("expires_in"):
        exp = datetime.now(timezone.utc) + timedelta(seconds=int(tok["expires_in"]))
        info["expiry"] = exp.strftime("%Y-%m-%dT%H:%M:%SZ")
    return info


def _is_expired(info):
    exp_raw = info.get("expiry")
    if not exp_raw or not isinstance(exp_raw, str):
        return False
    try:
        cleaned = exp_raw.replace("Z", "+00:00")
        exp = datetime.fromisoformat(cleaned)
        if exp.tzinfo is None:
            exp = exp.replace(tzinfo=timezone.utc)
        return datetime.now(timezone.utc) + timedelta(seconds=60) >= exp
    except ValueError:
        return False


def access(account, force=False):
    info = credential(account)
    if info.get("refresh_token") and (force or not info.get("token") or _is_expired(info)):
        info = _stdlib_refresh(info)
        raw = json.dumps(info)
        if account == "active":
            set_slots(raw, None)
        else:
            put("account:" + account, raw)
    if not info.get("token"):
        raise Failure("The credential needs a new sign-in.")
    return info


def email(info):
    id_token = info.get("id_token", "")
    if isinstance(id_token, str) and id_token.count(".") == 2:
        try:
            payload = id_token.split(".")[1]
            value = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
            if value.get("email"):
                return value["email"]
        except (ValueError, TypeError):
            pass
    query = urlencode({"access_token": info["token"]})
    with urlopen("https://oauth2.googleapis.com/tokeninfo?" + query, timeout=15) as response:
        return json.load(response).get("email")


def parse_xssi(data):
    text = data.decode("utf-8")
    if text.startswith(")]}'\n"):
        text = text[5:]
    return json.loads(text) if text.strip() else None


def request(account, path, xsrf=None, params=None, data=None, content_type=None):
    for retry in (False, True):
        headers = {"X-Colab-Client-Agent": "colab-cli", "Accept": "application/json"}
        if content_type:
            headers["Content-Type"] = content_type
        query = urlencode({"authuser": "0", **(params or {})})
        req = Request(TUN + path + "?" + query, headers=headers,
                      data=data if data is not None else b"" if xsrf is not None else None)
        req.add_unredirected_header("Authorization", "Bearer " + access(account, retry)["token"])
        if xsrf is not None:
            req.add_unredirected_header("X-Goog-Colab-Token", xsrf)
        try:
            with urlopen(req, timeout=30) as response:
                return parse_xssi(response.read())
        except HTTPError as exc:
            if exc.code != 401 or retry:
                raise


def begin_add(clear):
    if get("add-backup") is not None:
        raise Failure("An account sign-in is pending. Finish or roll it back first.")
    put("add-backup", json.dumps(slots()))
    if clear:
        try:
            set_slots(None, None)
        except Exception:
            rollback()
            raise


def rollback():
    saved = get("add-backup")
    if saved is not None:
        set_slots(**json.loads(saved))
        put("add-flow", None)
        put("add-backup", None)


def oauth_client_credentials():
    custom = Path.home() / ".colab-cli-oauth-config.json"
    if custom.exists():
        try:
            cfg = json.loads(custom.read_text(encoding="utf-8"))
            installed = cfg.get("installed") or cfg.get("web") or {}
            if installed.get("client_id") and installed.get("client_secret"):
                return installed["client_id"], installed["client_secret"]
        except Exception:
            pass
    return DEFAULT_CLIENT_ID, DEFAULT_CLIENT_SECRET


def build_pkce_login_url():
    client_id, _ = oauth_client_credentials()
    verifier = secrets.token_urlsafe(64)
    digest = hashlib.sha256(verifier.encode("ascii")).digest()
    challenge = base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")
    params = {
        "response_type": "code",
        "client_id": client_id,
        "redirect_uri": REMOTE_REDIRECT_URI,
        "scope": " ".join(PUBLIC_SCOPES),
        "access_type": "offline",
        "prompt": "consent",
        "token_usage": "remote",
        "code_challenge": challenge,
        "code_challenge_method": "S256",
    }
    return verifier, OAUTH_AUTH_URI + "?" + urlencode(params)


def exchange_pkce_code(raw_code, verifier):
    code = (raw_code or "").strip()
    if code.startswith("http://") or code.startswith("https://"):
        parsed_q = parse_qs(urlparse(code).query)
        if parsed_q.get("code"):
            code = parsed_q["code"][0].strip()
    if not code:
        raise Failure("请输入 Google 授权页面显示的授权码。")
    client_id, client_secret = oauth_client_credentials()
    body = urlencode({
        "code": code,
        "client_id": client_id,
        "client_secret": client_secret,
        "redirect_uri": REMOTE_REDIRECT_URI,
        "grant_type": "authorization_code",
        "code_verifier": verifier,
    }).encode("utf-8")
    req = Request(
        OAUTH_TOKEN_URI,
        data=body,
        headers={"Content-Type": "application/x-www-form-urlencoded"},
    )
    try:
        with urlopen(req, timeout=25) as resp:
            tok = json.load(resp)
    except HTTPError as exc:
        raise Failure(f"授权码无效或已过期 (HTTP {exc.code})，请重新点击打开授权页面获取新授权码。")
    if not tok.get("access_token"):
        raise Failure("Google 未返回有效的访问令牌，请重试。")
    scopes = tok.get("scope", "").split() or list(PUBLIC_SCOPES)
    cred = {
        "token": tok["access_token"],
        "refresh_token": tok.get("refresh_token"),
        "token_uri": OAUTH_TOKEN_URI,
        "client_id": client_id,
        "client_secret": client_secret,
        "scopes": scopes,
        "universe_domain": "googleapis.com",
        "account": "",
    }
    if tok.get("id_token"):
        cred["id_token"] = tok["id_token"]
    if tok.get("expires_in"):
        exp = datetime.now(timezone.utc) + timedelta(seconds=int(tok["expires_in"]))
        cred["expiry"] = exp.strftime("%Y-%m-%dT%H:%M:%SZ")
    return json.dumps(cred)


def dispatch(op, args):
    if op == "status":
        if not active():
            return {"active": False, "email": None}
        return {"active": True, "email": email(access("active"))}
    if op == "begin-add":
        begin_add(clear=True)
    elif op == "begin-login":
        if get("add-backup") is None:
            put("add-backup", json.dumps(slots()))
        try:
            verifier, url = build_pkce_login_url()
            put("add-flow", json.dumps({"verifier": verifier, "url": url}))
            return {"url": url}
        except Exception:
            rollback()
            raise
    elif op == "rollback-add":
        rollback()
    elif op in ("adopt", "finish-add", "complete-login"):
        if op != "adopt" and get("add-backup") is None:
            raise Failure("登录会话已重置，请重新点击「打开 Google 授权页」。")
        if op == "complete-login":
            flow_raw = get("add-flow")
            if not flow_raw:
                raise Failure("请先点击打开 Google 授权页面。")
            cred_json = exchange_pkce_code(sys.stdin.read(), json.loads(flow_raw)["verifier"])
            set_slots(cred_json, None)
        info = access("active")
        addr = email(info)
        if not addr:
            raise Failure("Google did not identify the signed-in account.")
        put("account:" + args[0], json.dumps(info))
        if op != "adopt":
            previous = json.loads(get("add-backup"))
            if previous["ring"] is not None or previous["file"] is not None:
                # The previous CLI login may belong to someone the desktop has
                # never adopted. The new account already has its own saved slot.
                set_slots(**previous)
            put("add-flow", None)
            put("add-backup", None)
        return {"email": addr}
    elif op == "use":
        if get("add-backup") is not None:
            raise Failure("Finish or roll back the pending sign-in first.")
        saved = get("account:" + args[0])
        if not saved:
            raise Failure("Saved account credential unavailable.")
        set_slots(saved, None)
    elif op == "remove":
        put("account:" + args[0], None)
    elif op == "usage":
        usage = request(args[0], "ccu-info")
        balance, rate = float(usage["currentBalance"]), float(usage["consumptionRateHourly"])
        if not all(map(math.isfinite, (balance, rate))):
            raise Failure("Invalid Colab usage response.")
        tier = str(usage.get("planTier", "")).upper().replace(" ", "_")
        high = tier in {"PRO+", "PRO_PLUS", "PROPLUS", "COLAB_PRO_PLUS", "HIGH"}
        return {"currentBalance": balance, "consumptionRateHourly": rate,
                "assignmentsCount": int(usage["assignmentsCount"]),
                "planTier": "PRO+" if high or balance >= 300 else "PRO" if balance > 0 else "FREE"}
    elif op == "assignments":
        rows = request(args[0] if args else "active", "assignments")["assignments"]
        return [{k: row[k] for k in ("endpoint", "accelerator", "variant", "machineShape")}
                for row in rows]
    elif op == "drive-consent-ready":
        account, endpoint = args
        path = "credentials-propagation/" + quote(endpoint, safe="")
        params = dict(authtype="dfs_ephemeral", version="2", dryrun="true",
                      propagate="true", record="false")
        token = request(account, path, params=params)["token"]
        body = (b'--nbx\r\nContent-Disposition: form-data; name="file_id"; '
                b'filename="file_id"\r\n\r\nempty.ipynb\r\n--nbx--\r\n')
        reply = request(account, path, xsrf=token, params=params, data=body,
                        content_type="multipart/form-data; boundary=nbx")
        return {"ready": reply.get("success") is True}
    elif op == "unassign":
        account = args[1] if len(args) > 1 else "active"
        path = "unassign/" + quote(args[0], safe="")
        token = request(account, path)["token"]
        request(account, path, xsrf=token)
    elif op.startswith("secret-"):
        key = "secret:" + args[0]
        if op == "secret-set":
            put(key, sys.stdin.read())
        elif op == "secret-get":
            return {"value": get(key)}
        elif op == "secret-has":
            return {"exists": get(key) is not None}
        elif op == "secret-delete":
            put(key, None)
        else:
            raise Failure("Unknown secret operation.")
    else:
        raise Failure("Unknown account operation.")
    return {"ok": True}


def run_scoped_cli(account, argv):
    # --config selects sessions only. Bind OAuth in this process too, so a
    # concurrent account switch can never direct 'new' or 'stop' to another user.
    op = argv[2]
    if op == "_forget-released":
        # This runs only after the assignment service confirmed release.
        # Recheck endpoint identity under the CLI writer's own lock.
        from colab_cli.state import StateStore
        store = StateStore(argv[1])
        name, endpoint = argv[3:]
        with store._lock_exclusive() as f:
            f.seek(0)
            records = json.loads(f.read() or "{}")
            if records.get(name, {}).get("endpoint") == endpoint:
                del records[name]
                store._write_data(f, json.dumps(records))
        return
    from colab_cli import auth
    from google.oauth2.credentials import Credentials

    def scoped_credentials(_config):
        with locked():
            return Credentials.from_authorized_user_info(access(account))

    auth._get_google_auth_credentials = scoped_credentials
    if op == "drivemount" and os.name == "posix":
        # Force the CLI's documented stdin fallback when launched from a terminal.
        if os.getsid(0) != os.getpid():
            os.setsid()
    if op == "ssh" and "--proxy-mode" in argv:
        from colab_cli.commands import ssh
        # Upstream normally auto-provisions when a named SSH session is absent.
        # A reconnect is allowed to attach only to the originally requested VM.
        def existing(session, *_args, **_kwargs):
            record = ssh._resolve_session(session)
            if not record or record.endpoint != os.environ["NBX_TUNNEL_ENDPOINT"]:
                raise Failure("SSH session is no longer assigned.")
            return record, False
        if not hasattr(ssh, "_select_proxy_session"):
            raise Failure("Unsupported Colab SSH implementation.")
        ssh._select_proxy_session = existing
    from colab_cli.cli import main
    if op in ("_upload-rclone", "_upload-env"):
        # Upload config as a file, keeping credentials out of kernel history,
        # command lines, stdout, and the environment inherited by the CLI.
        if op == "_upload-rclone":
            op, remote = "_upload-rclone", "root/.notebookx/rclone.conf"
            config = os.environ.pop("NBX_RCLONE_CONFIG")
        else:
            op, remote = "_upload-env", argv.pop()
            config = sys.stdin.read()
        fd, path = tempfile.mkstemp()
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(config)
            argv[2] = "upload"
            sys.argv = ["colab", *argv, path, remote]
            main()
        finally:
            os.unlink(path)
        return
    sys.argv = ["colab", *argv]
    main()


if __name__ == "__main__":
    if sys.argv[1:2] == ["_cli"]:
        run_scoped_cli(sys.argv[2], sys.argv[3:])
    else:
        logging.disable(logging.CRITICAL)
        failed = False
        try:
            with open(os.devnull, "w") as quiet, redirect_stdout(quiet), redirect_stderr(quiet), locked():
                try:
                    result = dispatch(sys.argv[1], sys.argv[2:])
                except Exception:
                    if sys.argv[1:2] in (["complete-login"], ["finish-add"]):
                        rollback()
                    raise
        except Exception as exc:
            failed = True
            message = str(exc) if isinstance(exc, Failure) else type(exc).__name__
            if isinstance(exc, HTTPError):
                message = "Google returned HTTP " + str(exc.code)
            result = {"error": message}
        print("__NBX_RESULT__ " + json.dumps(result, ensure_ascii=True, allow_nan=False))
        sys.exit(int(failed))
