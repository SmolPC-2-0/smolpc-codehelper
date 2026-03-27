"""
Shared MCP stdio client for end-to-end connector tests.

Spawns an MCP server as a subprocess, communicates via JSON-RPC over stdio.
"""
import json
import os
import subprocess
import sys
import time

RESPONSE_TIMEOUT = 15


class McpClient:
    """Minimal JSON-RPC client that talks to an MCP server over stdio."""

    def __init__(self, server_script: str, working_dir: str):
        env = os.environ.copy()
        env["SMOLPC_MCP_LOG_DIR"] = working_dir
        self.proc = subprocess.Popen(
            [sys.executable, server_script],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=working_dir,
            env=env,
        )
        self._next_id = 1
        self._initialize()

    def _send(self, method: str, params=None) -> dict:
        msg_id = self._next_id
        self._next_id += 1
        payload = {"jsonrpc": "2.0", "id": msg_id, "method": method}
        if params is not None:
            payload["params"] = params
        self.proc.stdin.write((json.dumps(payload) + "\n").encode())
        self.proc.stdin.flush()
        return self._recv(msg_id)

    def _notify(self, method: str, params=None):
        payload = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            payload["params"] = params
        self.proc.stdin.write((json.dumps(payload) + "\n").encode())
        self.proc.stdin.flush()

    def _recv(self, expected_id: int) -> dict:
        deadline = time.monotonic() + RESPONSE_TIMEOUT
        while time.monotonic() < deadline:
            line = self.proc.stdout.readline()
            if not line:
                stderr = (
                    self.proc.stderr.read().decode(errors="replace")
                    if self.proc.poll() is not None
                    else ""
                )
                raise RuntimeError(f"MCP server closed stdout. stderr: {stderr}")
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if msg.get("id") == expected_id:
                return msg
        raise TimeoutError(f"Timed out waiting for response id={expected_id}")

    def _initialize(self):
        resp = self._send("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test-e2e", "version": "1.0.0"},
        })
        assert "result" in resp, f"initialize failed: {resp}"
        self._notify("notifications/initialized")

    def list_tools(self) -> list:
        return self._send("tools/list")["result"]["tools"]

    def call_tool(self, name: str, arguments: dict) -> str:
        resp = self._send("tools/call", {"name": name, "arguments": arguments})
        if "error" in resp:
            raise RuntimeError(f"Tool call error: {resp['error']}")
        texts = [c["text"] for c in resp["result"]["content"] if c.get("type") == "text"]
        return "\n".join(texts)

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.stderr.close()
        except Exception:
            pass
        try:
            self.proc.stdout.close()
        except Exception:
            pass
        try:
            self.proc.terminate()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()
