#!/usr/bin/env python3

"""SmolPC TCP MCP bridge for the vendored GIMP plugin socket."""

from __future__ import annotations

import json
import os
import socket
import socketserver
import sys
import traceback
from typing import Any

BRIDGE_HOST = os.environ.get("SMOLPC_GIMP_BRIDGE_HOST", "127.0.0.1")
BRIDGE_PORT = int(os.environ.get("SMOLPC_GIMP_BRIDGE_PORT", "10008"))
PLUGIN_HOST = os.environ.get("SMOLPC_GIMP_PLUGIN_HOST", "127.0.0.1")
PLUGIN_PORT = int(os.environ.get("SMOLPC_GIMP_PLUGIN_PORT", "9877"))

SERVER_INFO = {
    "name": "smolpc-gimp-mcp-bridge",
    "version": "phase5",
}

TOOLS = [
    {
        "name": "get_image_bitmap",
        "description": "Return the current image bitmap or a selected region from GIMP.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "max_width": {"type": "integer", "minimum": 1},
                "max_height": {"type": "integer", "minimum": 1},
                "region": {
                    "type": "object",
                    "properties": {
                        "origin_x": {"type": "integer", "minimum": 0},
                        "origin_y": {"type": "integer", "minimum": 0},
                        "width": {"type": "integer", "minimum": 1},
                        "height": {"type": "integer", "minimum": 1},
                        "max_width": {"type": "integer", "minimum": 1},
                        "max_height": {"type": "integer", "minimum": 1},
                    },
                    "additionalProperties": False,
                },
            },
            "additionalProperties": False,
        },
    },
    {
        "name": "get_image_metadata",
        "description": "Return metadata about the current image in GIMP.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
    },
    {
        "name": "get_gimp_info",
        "description": "Return environment and installation details for the active GIMP instance.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
    },
    {
        "name": "get_context_state",
        "description": "Return the current brush, color, and context state from GIMP.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
    },
    {
        "name": "call_api",
        "description": "Execute GIMP API commands through the provisioned plugin socket.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "api_path": {"type": "string"},
                "args": {"type": "array"},
                "kwargs": {"type": "object"},
            },
            "required": ["api_path"],
            "additionalProperties": False,
        },
    },
]


def jsonrpc_success(message_id: Any, result: dict[str, Any]) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "id": message_id, "result": result}


def jsonrpc_error(message_id: Any, code: int, message: str) -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": message_id,
        "error": {"code": code, "message": message},
    }


def plugin_request(tool_name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    payload = {"type": tool_name, "params": arguments}
    encoded = json.dumps(payload).encode("utf-8")
    with socket.create_connection((PLUGIN_HOST, PLUGIN_PORT), timeout=5.0) as sock:
        sock.sendall(encoded)
        sock.shutdown(socket.SHUT_WR)
        chunks: list[bytes] = []
        while True:
            chunk = sock.recv(8192)
            if not chunk:
                break
            chunks.append(chunk)
    if not chunks:
        raise ConnectionError(
            f"GIMP plugin socket at {PLUGIN_HOST}:{PLUGIN_PORT} returned no response."
        )
    return json.loads(b"".join(chunks).decode("utf-8"))


def tool_result_from_plugin(tool_name: str, plugin_result: dict[str, Any]) -> dict[str, Any]:
    if plugin_result.get("status") != "success":
        error = plugin_result.get("error", "Unknown GIMP plugin error")
        return {
            "content": [{"type": "text", "text": f"Error: {error}"}],
            "isError": True,
            "structuredContent": {
                "status": "error",
                "error": error,
                "tool": tool_name,
            },
        }

    result = plugin_result.get("results")
    text = json.dumps(result)
    return {
        "content": [{"type": "text", "text": text}],
        "structuredContent": {
            "status": "success",
            "tool": tool_name,
            "result": result,
        },
    }


def handle_tool_call(name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    if name not in {tool["name"] for tool in TOOLS}:
        raise ValueError(f"Unsupported GIMP MCP tool: {name}")
    plugin_result = plugin_request(name, arguments)
    return tool_result_from_plugin(name, plugin_result)


class BridgeHandler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        while True:
            raw = self.rfile.readline()
            if not raw:
                return

            try:
                message = json.loads(raw.decode("utf-8").strip())
            except json.JSONDecodeError as error:
                response = jsonrpc_error(None, -32700, f"Invalid JSON-RPC payload: {error}")
                self.wfile.write((json.dumps(response) + "\n").encode("utf-8"))
                self.wfile.flush()
                continue

            method = message.get("method")
            message_id = message.get("id")
            params = message.get("params") or {}

            if method == "initialize":
                response = jsonrpc_success(
                    message_id,
                    {
                        "serverInfo": SERVER_INFO,
                        "capabilities": {"tools": {"listChanged": False}},
                    },
                )
            elif method == "notifications/initialized":
                continue
            elif method == "tools/list":
                response = jsonrpc_success(message_id, {"tools": TOOLS})
            elif method == "tools/call":
                try:
                    result = handle_tool_call(
                        params.get("name", ""),
                        params.get("arguments") or {},
                    )
                    response = jsonrpc_success(message_id, result)
                except Exception as error:  # pragma: no cover - surfaced to caller
                    traceback.print_exc(file=sys.stderr)
                    response = jsonrpc_error(message_id, -32000, str(error))
            else:
                response = jsonrpc_error(message_id, -32601, f"Unknown method: {method}")

            self.wfile.write((json.dumps(response) + "\n").encode("utf-8"))
            self.wfile.flush()


class ThreadedTcpServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main() -> None:
    with ThreadedTcpServer((BRIDGE_HOST, BRIDGE_PORT), BridgeHandler) as server:
        print(
            f"SmolPC GIMP MCP bridge listening on {BRIDGE_HOST}:{BRIDGE_PORT} -> "
            f"{PLUGIN_HOST}:{PLUGIN_PORT}",
            flush=True,
        )
        server.serve_forever()


if __name__ == "__main__":
    main()
