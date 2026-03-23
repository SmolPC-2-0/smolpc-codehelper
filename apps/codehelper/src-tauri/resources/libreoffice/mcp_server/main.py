#!/usr/bin/env python3
import atexit
import json
import logging
import os
import platform
import secrets
import signal
import socket
import subprocess
import shutil
import sys
import threading
import time
from pathlib import Path
from typing import IO, Optional

HELPER_PORT = 8765
OFFICE_PORT = 2002
STARTUP_TIMEOUT_SECONDS = 20.0
STARTUP_POLL_INTERVAL_SECONDS = 0.2
SHUTDOWN_TIMEOUT_SECONDS = 5.0
PORT_RELEASE_TIMEOUT_SECONDS = 6.0
PORT_RELEASE_POLL_INTERVAL_SECONDS = 0.2
HELPER_AUTH_TOKEN_ENV = "SMOLPC_LIBREOFFICE_HELPER_AUTH_TOKEN"
HELPER_AUTH_TOKEN_FIELD = "_smolpc_auth_token"
OFFICE_PATH_ENV = "SMOLPC_LIBREOFFICE_OFFICE_PATH"
HELPER_PYTHON_PATH_ENV = "SMOLPC_LIBREOFFICE_HELPER_PYTHON_PATH"
MCP_LOG_DIR_ENV = "SMOLPC_MCP_LOG_DIR"
HELPER_STDERR_LOG_FILENAME = "helper.stderr.log"
OFFICE_PROFILE_DIR_PREFIX = "office-profile-"
RUNTIME_WATCHDOG_POLL_INTERVAL_SECONDS = 0.5
RUNTIME_WATCHDOG_RECOVERY_COOLDOWN_SECONDS = 2.0

office_process: Optional[subprocess.Popen] = None
helper_process: Optional[subprocess.Popen] = None
server_process: Optional[subprocess.Popen] = None
helper_stderr_handle: Optional[IO[str]] = None
office_profile_dir: Optional[Path] = None
process_state_lock = threading.RLock()
runtime_watchdog_stop_event = threading.Event()
runtime_watchdog_thread: Optional[threading.Thread] = None


def configure_logging() -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(message)s",
        stream=sys.stderr,
    )


def stop_process(label: str, process: Optional[subprocess.Popen]) -> None:
    if process is None or process.poll() is not None:
        return

    logging.info("Stopping %s process", label)
    process.terminate()
    try:
        process.wait(timeout=SHUTDOWN_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        logging.warning("%s process did not exit after terminate(); killing it", label)
        process.kill()
        try:
            process.wait(timeout=SHUTDOWN_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            logging.error("%s process did not exit after kill()", label)


def is_port_listening(port: int) -> bool:
    pids = list_listening_pids(port)
    if pids:
        return True
    if platform.system() == "Windows":
        return False
    return is_port_in_use(port)


def helper_runtime_unhealthy_reason() -> Optional[str]:
    with process_state_lock:
        tracked_helper = helper_process

    if is_port_listening(HELPER_PORT):
        return None

    if tracked_helper is not None:
        return_code = tracked_helper.poll()
        if return_code is not None:
            return f"helper process exited with code {return_code}"

    return f"helper socket is unavailable on port {HELPER_PORT}"


def office_runtime_unhealthy_reason() -> Optional[str]:
    with process_state_lock:
        tracked_office = office_process

    if is_port_listening(OFFICE_PORT):
        return None

    if tracked_office is not None:
        return_code = tracked_office.poll()
        if return_code is not None:
            return f"office process exited with code {return_code}"

    return f"office socket is unavailable on port {OFFICE_PORT}"


def recover_runtime_after_failure(
    helper_token: str,
    reason: str,
    recover_helper: bool,
    recover_office: bool,
) -> None:
    logging.warning(
        "Runtime watchdog detected instability (%s); restarting runtime components.",
        reason,
    )
    if recover_office:
        start_office(force_restart=True)
    if recover_helper:
        start_helper(helper_token)
    if recover_helper:
        try:
            wait_for_helper_desktop_ready(helper_token)
        except TimeoutError:
            logging.warning(
                "Desktop readiness check failed after helper recovery; restarting helper once and retrying.",
                exc_info=True,
            )
            start_helper(helper_token)
            wait_for_helper_desktop_ready(helper_token)
    elif recover_office:
        logging.info(
            "Office recovery completed; skipping helper readiness probe to avoid interrupting in-flight commands."
        )


def runtime_watchdog_loop(helper_token: str) -> None:
    next_recovery_allowed_at = 0.0
    while not runtime_watchdog_stop_event.is_set():
        try:
            helper_reason = helper_runtime_unhealthy_reason()
            office_reason = office_runtime_unhealthy_reason()

            if helper_reason or office_reason:
                now = time.monotonic()
                if now >= next_recovery_allowed_at:
                    reason = helper_reason or office_reason or "unknown runtime instability"
                    recover_runtime_after_failure(
                        helper_token,
                        reason,
                        recover_helper=helper_reason is not None,
                        recover_office=office_reason is not None,
                    )
                    next_recovery_allowed_at = (
                        time.monotonic()
                        + RUNTIME_WATCHDOG_RECOVERY_COOLDOWN_SECONDS
                    )
            else:
                next_recovery_allowed_at = 0.0
        except Exception:
            logging.exception(
                "Runtime watchdog encountered an error while attempting recovery"
            )
            next_recovery_allowed_at = (
                time.monotonic() + RUNTIME_WATCHDOG_RECOVERY_COOLDOWN_SECONDS
            )

        runtime_watchdog_stop_event.wait(RUNTIME_WATCHDOG_POLL_INTERVAL_SECONDS)


def start_runtime_watchdog(helper_token: str) -> None:
    global runtime_watchdog_thread
    if runtime_watchdog_thread is not None and runtime_watchdog_thread.is_alive():
        return

    runtime_watchdog_stop_event.clear()
    runtime_watchdog_thread = threading.Thread(
        target=runtime_watchdog_loop,
        args=(helper_token,),
        daemon=True,
        name="libreoffice-runtime-watchdog",
    )
    runtime_watchdog_thread.start()
    logging.info("Started LibreOffice runtime watchdog thread")


def stop_runtime_watchdog() -> None:
    global runtime_watchdog_thread
    runtime_watchdog_stop_event.set()

    if runtime_watchdog_thread is not None and runtime_watchdog_thread.is_alive():
        if threading.current_thread() is not runtime_watchdog_thread:
            runtime_watchdog_thread.join(timeout=SHUTDOWN_TIMEOUT_SECONDS)
    runtime_watchdog_thread = None


def cleanup_processes() -> None:
    global office_process, helper_process, server_process, helper_stderr_handle, office_profile_dir

    stop_runtime_watchdog()
    with process_state_lock:
        stop_process("MCP server", server_process)
        stop_process("LibreOffice helper", helper_process)
        stop_process("LibreOffice office", office_process)

        if helper_stderr_handle is not None:
            try:
                helper_stderr_handle.close()
            except OSError:
                logging.warning("Unable to close helper stderr log handle", exc_info=True)

        if office_profile_dir is not None:
            try:
                shutil.rmtree(office_profile_dir, ignore_errors=True)
            except OSError:
                logging.warning(
                    "Unable to remove office profile directory %s", office_profile_dir
                )

        server_process = None
        helper_process = None
        office_process = None
        helper_stderr_handle = None
        office_profile_dir = None


def signal_handler(signum, _frame) -> None:
    logging.info("Received signal %s; shutting down LibreOffice MCP runtime", signum)
    stop_runtime_watchdog()
    cleanup_processes()
    raise SystemExit(0)


def register_shutdown_handlers() -> None:
    atexit.register(cleanup_processes)
    signal.signal(signal.SIGINT, signal_handler)
    if platform.system() != "Windows":
        signal.signal(signal.SIGTERM, signal_handler)


def get_office_path() -> str:
    configured_path = os.environ.get(OFFICE_PATH_ENV)
    if configured_path:
        if os.path.exists(configured_path):
            return configured_path
        raise FileNotFoundError(
            f"Configured LibreOffice executable from {OFFICE_PATH_ENV} does not exist: {configured_path}"
        )

    system = platform.system().lower()
    if system == "windows":
        possible_paths = [
            r"C:\Program Files\Collabora Office\program\soffice.exe",
            r"C:\Program Files (x86)\Collabora Office\program\soffice.exe",
            r"C:\Program Files\LibreOffice\program\soffice.exe",
        ]
    elif system == "linux":
        possible_paths = [
            "/usr/bin/coolwsd",
            "/usr/bin/collaboraoffice",
            "/opt/collaboraoffice/program/soffice",
            "/usr/lib/collaboraoffice/program/soffice",
        ]
    else:
        raise OSError(f"Unsupported operating system: {system}")

    for path in possible_paths:
        if os.path.exists(path):
            return path
    raise FileNotFoundError(
        "Neither Collabora Office nor LibreOffice executable found. Please install either office suite."
    )


def get_python_path(soffice_path: str) -> str:
    configured_python = os.environ.get(HELPER_PYTHON_PATH_ENV)
    if configured_python:
        if os.path.isfile(configured_python):
            logging.info(
                "Using helper Python from %s: %s",
                HELPER_PYTHON_PATH_ENV,
                configured_python,
            )
            return configured_python
        raise FileNotFoundError(
            f"Configured helper Python from {HELPER_PYTHON_PATH_ENV} is not a file: {configured_python}"
        )

    office_program_dir = Path(soffice_path).resolve().parent
    system = platform.system().lower()
    candidate_paths = []

    if system == "windows":
        candidate_paths.append(office_program_dir / "python.exe")
        for python_core_dir in sorted(
            office_program_dir.glob("python-core-*"), reverse=True
        ):
            candidate_paths.append(python_core_dir / "bin" / "python.exe")
    else:
        candidate_paths.append(office_program_dir / "python3")
        candidate_paths.append(office_program_dir / "python")
        for python_core_dir in sorted(
            office_program_dir.glob("python-core-*"), reverse=True
        ):
            candidate_paths.append(python_core_dir / "bin" / "python3")
            candidate_paths.append(python_core_dir / "bin" / "python")
        candidate_paths.append(office_program_dir.parent / "Resources" / "python")

    for candidate_path in candidate_paths:
        if candidate_path.is_file():
            candidate = str(candidate_path)
            logging.info(
                "Using LibreOffice Python for helper startup: %s",
                candidate,
            )
            return candidate

    fallback = sys.executable
    logging.warning(
        "Unable to locate LibreOffice Python next to %s; falling back to runtime interpreter %s. Set %s to override.",
        soffice_path,
        fallback,
        HELPER_PYTHON_PATH_ENV,
    )
    return fallback


def resolve_runtime_dir() -> Path:
    configured_dir = os.getenv(MCP_LOG_DIR_ENV)
    if configured_dir:
        try:
            log_dir = Path(configured_dir).expanduser()
            if not log_dir.is_absolute():
                raise ValueError(f"{MCP_LOG_DIR_ENV} must be an absolute path")
            log_dir.mkdir(parents=True, exist_ok=True)
            return log_dir.resolve(strict=True)
        except (OSError, ValueError):
            logging.warning(
                "Unable to use %s=%s for runtime directory; falling back to module directory",
                MCP_LOG_DIR_ENV,
                configured_dir,
            )

    module_dir = Path(__file__).resolve().parent
    module_dir.mkdir(parents=True, exist_ok=True)
    return module_dir


def resolve_log_path(filename: str) -> Path:
    return resolve_runtime_dir() / filename


def is_port_in_use(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(0.5)
        return sock.connect_ex(("127.0.0.1", port)) == 0


def list_listening_pids(port: int) -> set[int]:
    if platform.system() != "Windows":
        return set()

    result = subprocess.run(
        ["netstat", "-ano", "-p", "tcp"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        logging.warning(
            "netstat failed while checking port %s: %s",
            port,
            (result.stderr or result.stdout).strip(),
        )
        return set()

    pids: set[int] = set()
    for line in result.stdout.splitlines():
        parts = line.split()
        if len(parts) < 5:
            continue
        if parts[0].upper() != "TCP":
            continue
        if parts[3].upper() != "LISTENING":
            continue
        if not parts[1].endswith(f":{port}"):
            continue
        try:
            pid = int(parts[4])
        except ValueError:
            continue
        if pid > 0 and pid != os.getpid():
            pids.add(pid)

    return pids


def terminate_pid(pid: int, reason: str) -> None:
    if pid <= 0 or pid == os.getpid():
        return

    if platform.system() == "Windows":
        result = subprocess.run(
            ["taskkill", "/PID", str(pid), "/F", "/T"],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            output = f"{result.stdout}\n{result.stderr}".strip()
            normalized = output.lower()
            if "not found" in normalized or "no running instance" in normalized:
                return
            logging.warning(
                "taskkill failed for PID %s while %s: %s",
                pid,
                reason,
                output,
            )
        return

    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    except OSError:
        logging.warning("Unable to terminate PID %s while %s", pid, reason, exc_info=True)


def ensure_port_available(port: int, label: str) -> None:
    if not is_port_in_use(port):
        return

    pids = list_listening_pids(port)
    if not pids:
        raise RuntimeError(
            f"{label} is already in use on port {port}, and no owning PID could be identified."
        )

    logging.warning(
        "%s already occupied on port %s by PID(s) %s; terminating stale owner(s).",
        label,
        port,
        sorted(pids),
    )
    for pid in sorted(pids):
        terminate_pid(pid, f"releasing {label} on port {port}")

    deadline = time.monotonic() + PORT_RELEASE_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if not is_port_in_use(port):
            logging.info("%s is now available on port %s", label, port)
            return
        time.sleep(PORT_RELEASE_POLL_INTERVAL_SECONDS)

    raise TimeoutError(
        f"{label} remained in use on port {port} after stale owner termination attempts"
    )


def create_office_profile_dir() -> Path:
    runtime_dir = resolve_runtime_dir()
    profile_dir = runtime_dir / f"{OFFICE_PROFILE_DIR_PREFIX}{os.getpid()}-{secrets.token_hex(4)}"
    profile_dir.mkdir(parents=True, exist_ok=False)
    return profile_dir


def _strip_windows_extended_prefix(path: Path) -> Path:
    raw = str(path)
    if raw.startswith("\\\\?\\UNC\\"):
        return Path("\\\\" + raw[8:])
    if raw.startswith("\\\\?\\"):
        return Path(raw[4:])
    return path


def path_to_file_uri(path: Path) -> str:
    normalized = path
    if platform.system() == "Windows":
        normalized = _strip_windows_extended_prefix(path)
    return normalized.absolute().as_uri()


def wait_for_port_ready(
    port: int,
    label: str,
    process: Optional[subprocess.Popen] = None,
    failure_hint: Optional[str] = None,
) -> None:
    deadline = time.monotonic() + STARTUP_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if is_port_in_use(port):
            logging.info("%s is ready on port %s", label, port)
            return
        if process is not None:
            return_code = process.poll()
            if return_code is not None:
                hint_suffix = f" {failure_hint}" if failure_hint else ""
                raise RuntimeError(
                    f"{label} exited before becoming ready on port {port} (exit code {return_code}).{hint_suffix}"
                )
        time.sleep(STARTUP_POLL_INTERVAL_SECONDS)
    raise TimeoutError(f"{label} did not become ready on port {port}")


def recv_exact(stream: socket.socket, size: int) -> bytes:
    data = bytearray()
    while len(data) < size:
        chunk = stream.recv(size - len(data))
        if not chunk:
            break
        data.extend(chunk)
    return bytes(data)


def send_helper_command(command: dict, helper_token: str) -> dict:
    payload = dict(command)
    payload[HELPER_AUTH_TOKEN_FIELD] = helper_token
    encoded = json.dumps(payload).encode("utf-8")
    frame = len(encoded).to_bytes(4, byteorder="big") + encoded

    with socket.create_connection(("127.0.0.1", HELPER_PORT), timeout=5) as stream:
        stream.settimeout(5)
        stream.sendall(frame)
        header = recv_exact(stream, 4)
        if len(header) != 4:
            raise RuntimeError("helper response missing frame length header")
        message_length = int.from_bytes(header, byteorder="big")
        if message_length <= 0:
            raise RuntimeError("helper response returned an invalid frame length")
        response_payload = recv_exact(stream, message_length)
        if len(response_payload) != message_length:
            raise RuntimeError("helper response payload was truncated")
        decoded = json.loads(response_payload.decode("utf-8"))
        if not isinstance(decoded, dict):
            raise RuntimeError("helper response must be a JSON object")
        return decoded


def wait_for_helper_desktop_ready(helper_token: str) -> None:
    deadline = time.monotonic() + STARTUP_TIMEOUT_SECONDS
    last_error = ""
    while time.monotonic() < deadline:
        try:
            response = send_helper_command({"action": "desktop_ready"}, helper_token)
            status = response.get("status")
            if status == "success":
                logging.info("Helper confirmed LibreOffice desktop readiness")
                return
            last_error = str(response.get("error") or response)
        except Exception as error:  # noqa: BLE001
            last_error = str(error)
        time.sleep(STARTUP_POLL_INTERVAL_SECONDS)

    raise TimeoutError(
        f"Helper could not confirm LibreOffice desktop readiness before timeout: {last_error}"
    )


def start_office(port: int = OFFICE_PORT, force_restart: bool = False) -> None:
    global office_process, office_profile_dir
    with process_state_lock:
        if force_restart:
            ensure_port_available(port, "LibreOffice office socket")
        elif is_port_in_use(port):
            logging.info(
                "Office socket already available on port %s; reusing existing office process",
                port,
            )
            office_process = None
            return

        if office_process is not None and office_process.poll() is None:
            stop_process("LibreOffice office", office_process)
        office_process = None

        if office_profile_dir is not None:
            try:
                shutil.rmtree(office_profile_dir, ignore_errors=True)
            except OSError:
                logging.warning(
                    "Unable to remove previous office profile directory %s", office_profile_dir
                )
            office_profile_dir = None

        soffice_path = get_office_path()
        office_profile_dir = create_office_profile_dir()
        profile_uri = path_to_file_uri(office_profile_dir)
        logging.info("Starting headless office process from %s", soffice_path)
        logging.info("Using isolated LibreOffice profile %s", office_profile_dir)
        office_process = subprocess.Popen(
            [
                soffice_path,
                f"-env:UserInstallation={profile_uri}",
                "--headless",
                f"--accept=socket,host=localhost,port={port};urp;",
                "--norestore",
                "--nodefault",
                "--nologo",
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    wait_for_port_ready(port, "LibreOffice office socket", process=office_process)


def build_child_env(helper_token: str) -> dict:
    env = os.environ.copy()
    env[HELPER_AUTH_TOKEN_ENV] = helper_token
    return env


def start_helper(helper_token: str, port: int = HELPER_PORT) -> None:
    global helper_process, helper_stderr_handle

    with process_state_lock:
        ensure_port_available(port, "LibreOffice helper socket")

        helper_script = Path(__file__).with_name("helper.py")
        python_path = get_python_path(get_office_path())
        logging.info("Starting LibreOffice helper from %s", helper_script)
        helper_stderr_path = resolve_log_path(HELPER_STDERR_LOG_FILENAME)
        logging.info("Capturing LibreOffice helper stderr to %s", helper_stderr_path)
        helper_stderr_handle = open(helper_stderr_path, "a", encoding="utf-8", buffering=1)
        try:
            helper_process = subprocess.Popen(
                [python_path, str(helper_script)],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=helper_stderr_handle,
                env=build_child_env(helper_token),
            )
        except Exception:
            helper_stderr_handle.close()
            helper_stderr_handle = None
            raise
    wait_for_port_ready(
        port,
        "LibreOffice helper socket",
        process=helper_process,
        failure_hint="Check helper.log and helper.stderr.log for UNO import errors.",
    )


def start_mcp_server(helper_token: str) -> int:
    global server_process

    server_script = Path(__file__).with_name("libre.py")
    logging.info("Starting LibreOffice MCP server from %s", server_script)
    with process_state_lock:
        server_process = subprocess.Popen(
            [sys.executable, str(server_script)],
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
            env=build_child_env(helper_token),
        )
        tracked_server = server_process
    return tracked_server.wait()


def main() -> int:
    configure_logging()
    register_shutdown_handlers()
    helper_token = secrets.token_hex(32)
    stop_runtime_watchdog()
    logging.info("Starting LibreOffice MCP runtime")

    try:
        start_office()
        start_helper(helper_token)
        try:
            wait_for_helper_desktop_ready(helper_token)
        except TimeoutError:
            logging.warning(
                "Initial helper desktop readiness check failed; forcing office restart and retrying once",
                exc_info=True,
            )
            start_office(force_restart=True)
            wait_for_helper_desktop_ready(helper_token)
        start_runtime_watchdog(helper_token)
        return start_mcp_server(helper_token)
    except (FileNotFoundError, OSError, RuntimeError, TimeoutError, subprocess.SubprocessError):
        logging.exception("LibreOffice MCP runtime failed during startup or execution")
        return 1
    finally:
        cleanup_processes()


if __name__ == "__main__":
    raise SystemExit(main())
