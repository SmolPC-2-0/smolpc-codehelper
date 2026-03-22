#!/usr/bin/env python3
import atexit
import logging
import os
import platform
import secrets
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional, TextIO

HELPER_PORT = 8765
OFFICE_PORT = 2002
STARTUP_TIMEOUT_SECONDS = 20.0
STARTUP_POLL_INTERVAL_SECONDS = 0.2
SHUTDOWN_TIMEOUT_SECONDS = 5.0
HELPER_AUTH_TOKEN_ENV = "SMOLPC_LIBREOFFICE_HELPER_AUTH_TOKEN"
OFFICE_PATH_ENV = "SMOLPC_LIBREOFFICE_OFFICE_PATH"
HELPER_PYTHON_PATH_ENV = "SMOLPC_LIBREOFFICE_HELPER_PYTHON_PATH"
MCP_LOG_DIR_ENV = "SMOLPC_MCP_LOG_DIR"
HELPER_STDERR_LOG_FILENAME = "helper.stderr.log"

office_process: Optional[subprocess.Popen] = None
helper_process: Optional[subprocess.Popen] = None
server_process: Optional[subprocess.Popen] = None
helper_stderr_handle: Optional[TextIO] = None


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


def cleanup_processes() -> None:
    global office_process, helper_process, server_process, helper_stderr_handle

    stop_process("MCP server", server_process)
    stop_process("LibreOffice helper", helper_process)
    stop_process("LibreOffice office", office_process)

    if helper_stderr_handle is not None:
        try:
            helper_stderr_handle.close()
        except OSError:
            logging.warning("Unable to close helper stderr log handle", exc_info=True)

    server_process = None
    helper_process = None
    office_process = None
    helper_stderr_handle = None


def signal_handler(signum, _frame) -> None:
    logging.info("Received signal %s; shutting down LibreOffice MCP runtime", signum)
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
        if os.path.exists(configured_python):
            logging.info(
                "Using helper Python from %s: %s",
                HELPER_PYTHON_PATH_ENV,
                configured_python,
            )
            return configured_python
        raise FileNotFoundError(
            f"Configured helper Python from {HELPER_PYTHON_PATH_ENV} does not exist: {configured_python}"
        )

    office_program_dir = Path(soffice_path).resolve().parent
    system = platform.system().lower()
    candidate_paths = []

    if system == "windows":
        candidate_paths.append(office_program_dir / "python.exe")
        for python_core_dir in office_program_dir.glob("python-core-*"):
            candidate_paths.append(python_core_dir / "bin" / "python.exe")
    else:
        candidate_paths.append(office_program_dir / "python3")
        candidate_paths.append(office_program_dir / "python")
        for python_core_dir in office_program_dir.glob("python-core-*"):
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
        "Unable to locate LibreOffice Python next to %s; falling back to runtime interpreter %s",
        soffice_path,
        fallback,
    )
    return fallback


def resolve_log_path(filename: str) -> Path:
    configured_dir = os.getenv(MCP_LOG_DIR_ENV)
    if configured_dir:
        try:
            log_dir = Path(configured_dir).expanduser()
            if not log_dir.is_absolute():
                raise ValueError(f"{MCP_LOG_DIR_ENV} must be an absolute path")
            log_dir.mkdir(parents=True, exist_ok=True)
            return log_dir.resolve(strict=True) / filename
        except (OSError, RuntimeError, ValueError):
            logging.warning(
                "Unable to use %s=%s for log output; falling back to runtime directory",
                MCP_LOG_DIR_ENV,
                configured_dir,
            )

    module_dir = Path(__file__).resolve().parent
    module_dir.mkdir(parents=True, exist_ok=True)
    return module_dir / filename


def is_port_in_use(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(0.5)
        return sock.connect_ex(("127.0.0.1", port)) == 0


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


def start_office(port: int = OFFICE_PORT) -> None:
    global office_process

    if is_port_in_use(port):
        logging.info("Office socket already available on port %s", port)
        return

    soffice_path = get_office_path()
    logging.info("Starting headless office process from %s", soffice_path)
    office_process = subprocess.Popen(
        [
            soffice_path,
            "-env:UserInstallation=file:///C:/Temp/LibreOfficeHeadlessProfile",
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
    wait_for_port_ready(port, "LibreOffice office socket")


def build_child_env(helper_token: str) -> dict:
    env = os.environ.copy()
    env[HELPER_AUTH_TOKEN_ENV] = helper_token
    return env


def start_helper(helper_token: str, port: int = HELPER_PORT) -> None:
    global helper_process, helper_stderr_handle

    if is_port_in_use(port):
        raise RuntimeError(
            f"LibreOffice helper port {port} is already in use; refusing to talk to an unknown helper process."
        )

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
    server_process = subprocess.Popen(
        [sys.executable, str(server_script)],
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        env=build_child_env(helper_token),
    )
    return server_process.wait()


def main() -> int:
    configure_logging()
    register_shutdown_handlers()
    helper_token = secrets.token_hex(32)
    logging.info("Starting LibreOffice MCP runtime")

    try:
        start_office()
        start_helper(helper_token)
        return start_mcp_server(helper_token)
    except (FileNotFoundError, OSError, RuntimeError, TimeoutError, subprocess.SubprocessError):
        logging.exception("LibreOffice MCP runtime failed during startup or execution")
        return 1
    finally:
        cleanup_processes()


if __name__ == "__main__":
    raise SystemExit(main())
