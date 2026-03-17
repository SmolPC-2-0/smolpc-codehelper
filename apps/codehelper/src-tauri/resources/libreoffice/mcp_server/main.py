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
from typing import Optional

HELPER_PORT = 8765
OFFICE_PORT = 2002
STARTUP_TIMEOUT_SECONDS = 20.0
STARTUP_POLL_INTERVAL_SECONDS = 0.2
SHUTDOWN_TIMEOUT_SECONDS = 5.0
HELPER_AUTH_TOKEN_ENV = "SMOLPC_LIBREOFFICE_HELPER_AUTH_TOKEN"
OFFICE_PATH_ENV = "SMOLPC_LIBREOFFICE_OFFICE_PATH"

office_process: Optional[subprocess.Popen] = None
helper_process: Optional[subprocess.Popen] = None
server_process: Optional[subprocess.Popen] = None


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
    global office_process, helper_process, server_process

    stop_process("MCP server", server_process)
    stop_process("LibreOffice helper", helper_process)
    stop_process("LibreOffice office", office_process)

    server_process = None
    helper_process = None
    office_process = None


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


def get_python_path() -> str:
    return sys.executable


def is_port_in_use(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(0.5)
        return sock.connect_ex(("127.0.0.1", port)) == 0


def wait_for_port_ready(port: int, label: str) -> None:
    deadline = time.monotonic() + STARTUP_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if is_port_in_use(port):
            logging.info("%s is ready on port %s", label, port)
            return
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
    global helper_process

    if is_port_in_use(port):
        raise RuntimeError(
            f"LibreOffice helper port {port} is already in use; refusing to talk to an unknown helper process."
        )

    helper_script = Path(__file__).with_name("helper.py")
    python_path = get_python_path()
    logging.info("Starting LibreOffice helper from %s", helper_script)
    helper_process = subprocess.Popen(
        [python_path, str(helper_script)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=build_child_env(helper_token),
    )
    wait_for_port_ready(port, "LibreOffice helper socket")


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
