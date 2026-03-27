#!/usr/bin/env python3
"""
End-to-end tests for the LibreOffice MCP server.

Spawns the real mcp_server.py as a subprocess over stdio, sends JSON-RPC
tool calls, and verifies that documents are created and manipulated correctly.

No running LibreOffice instance is needed — the server uses python-docx,
python-pptx, and odfdo for headless document manipulation.

Usage:
    python test_e2e.py
"""
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

# Timeout for reading a single JSON-RPC response (seconds)
RESPONSE_TIMEOUT = 15


class McpClient:
    """Minimal JSON-RPC client that talks to the MCP server over stdio."""

    def __init__(self, working_dir: str):
        server_script = str(Path(__file__).resolve().parent / "mcp_server.py")
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
        raw = json.dumps(payload) + "\n"
        self.proc.stdin.write(raw.encode())
        self.proc.stdin.flush()
        return self._recv(msg_id)

    def _notify(self, method: str, params=None):
        payload = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            payload["params"] = params
        raw = json.dumps(payload) + "\n"
        self.proc.stdin.write(raw.encode())
        self.proc.stdin.flush()

    def _recv(self, expected_id: int) -> dict:
        deadline = time.monotonic() + RESPONSE_TIMEOUT
        while time.monotonic() < deadline:
            line = self.proc.stdout.readline()
            if not line:
                stderr_out = self.proc.stderr.read().decode(errors="replace") if self.proc.poll() is not None else ""
                raise RuntimeError(f"MCP server closed stdout. stderr: {stderr_out}")
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
        resp = self._send("tools/list")
        return resp["result"]["tools"]

    def call_tool(self, name: str, arguments: dict) -> str:
        resp = self._send("tools/call", {"name": name, "arguments": arguments})
        if "error" in resp:
            raise RuntimeError(f"Tool call error: {resp['error']}")
        content = resp["result"]["content"]
        texts = [c["text"] for c in content if c.get("type") == "text"]
        return "\n".join(texts)

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.terminate()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()


class TestMcpServerToolDiscovery(unittest.TestCase):
    """Tests for MCP tool listing and server startup."""

    @classmethod
    def setUpClass(cls):
        cls.tmpdir = tempfile.mkdtemp(prefix="smolpc_mcp_test_")
        cls.client = McpClient(cls.tmpdir)

    @classmethod
    def tearDownClass(cls):
        cls.client.close()

    def test_server_starts_and_lists_tools(self):
        tools = self.client.list_tools()
        self.assertGreater(len(tools), 0, "Server should expose at least one tool")
        tool_names = [t["name"] for t in tools]
        self.assertIn("create_blank_document", tool_names)
        self.assertIn("create_blank_presentation", tool_names)
        self.assertIn("read_text_document", tool_names)
        self.assertIn("read_presentation", tool_names)
        self.assertIn("add_heading", tool_names)
        self.assertIn("add_slide", tool_names)

    def test_writer_tools_present(self):
        tools = self.client.list_tools()
        tool_names = {t["name"] for t in tools}
        expected_writer = [
            "create_blank_document", "read_text_document",
            "add_text", "add_heading", "add_paragraph",
            "add_table", "format_text", "search_replace_text",
            "delete_text", "delete_paragraph", "insert_page_break",
            "apply_document_style", "format_table", "insert_image",
        ]
        for name in expected_writer:
            self.assertIn(name, tool_names, f"Writer tool '{name}' missing")

    def test_slides_tools_present(self):
        tools = self.client.list_tools()
        tool_names = {t["name"] for t in tools}
        expected_slides = [
            "create_blank_presentation", "read_presentation",
            "add_slide", "edit_slide_content", "edit_slide_title",
            "delete_slide", "format_slide_content", "format_slide_title",
            "apply_presentation_template", "insert_slide_image",
        ]
        for name in expected_slides:
            self.assertIn(name, tool_names, f"Slides tool '{name}' missing")


class TestWriterTools(unittest.TestCase):
    """End-to-end Writer tool tests using a fresh MCP server session."""

    @classmethod
    def setUpClass(cls):
        cls.tmpdir = tempfile.mkdtemp(prefix="smolpc_mcp_writer_")
        cls.docs_dir = os.path.join(cls.tmpdir, "Documents")
        os.makedirs(cls.docs_dir, exist_ok=True)
        cls.client = McpClient(cls.tmpdir)

    @classmethod
    def tearDownClass(cls):
        cls.client.close()

    def _doc_path(self, filename: str) -> str:
        return os.path.join(self.docs_dir, filename)

    # -- Document creation ---------------------------------------------------

    def test_create_blank_docx(self):
        path = self._doc_path("test-create.docx")
        result = self.client.call_tool("create_blank_document", {
            "filename": path,
            "title": "Test Document",
            "author": "SmolPC Test",
        })
        self.assertIn("Successfully created", result)
        self.assertTrue(os.path.exists(path), f"File should exist at {path}")

    def test_create_blank_odt(self):
        path = self._doc_path("test-create.odt")
        result = self.client.call_tool("create_blank_document", {
            "filename": path,
            "title": "ODT Test",
        })
        self.assertIn("Successfully created", result)
        self.assertTrue(os.path.exists(path))

    # -- Read document -------------------------------------------------------

    def test_read_text_document_docx(self):
        path = self._doc_path("test-read.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        self.client.call_tool("add_heading", {
            "file_path": path, "text": "Hello World", "level": 1
        })
        result = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("Hello World", result)

    def test_read_text_document_odt(self):
        path = self._doc_path("test-read.odt")
        self.client.call_tool("create_blank_document", {"filename": path})
        self.client.call_tool("add_paragraph", {
            "file_path": path, "text": "ODT paragraph content"
        })
        result = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("ODT paragraph", result)

    def test_read_odt_includes_headings(self):
        """Verify that read_text_document includes heading text for ODT files."""
        path = self._doc_path("test-read-heading.odt")
        self.client.call_tool("create_blank_document", {"filename": path})
        self.client.call_tool("add_heading", {
            "file_path": path, "text": "ODT Heading", "level": 1
        })
        result = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("ODT Heading", result)

    # -- Add heading ---------------------------------------------------------

    def test_add_heading(self):
        path = self._doc_path("test-heading.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        result = self.client.call_tool("add_heading", {
            "file_path": path, "text": "Chapter One", "level": 1
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("Chapter One", content)

    # -- Add paragraph -------------------------------------------------------

    def test_add_paragraph(self):
        path = self._doc_path("test-paragraph.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        result = self.client.call_tool("add_paragraph", {
            "file_path": path,
            "text": "This is a test paragraph with some content.",
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("test paragraph", content)

    # -- Add text ------------------------------------------------------------

    def test_add_text(self):
        path = self._doc_path("test-add-text.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        result = self.client.call_tool("add_text", {
            "file_path": path,
            "text": "Some inline text.",
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("inline text", content)

    # -- Add table -----------------------------------------------------------

    def test_add_table(self):
        path = self._doc_path("test-table.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        result = self.client.call_tool("add_table", {
            "file_path": path,
            "rows": 3,
            "columns": 2,
            "data": [["Name", "Score"], ["Alice", "95"], ["Bob", "87"]],
        })
        self.assertNotIn("Error", result)
        # Use test verification tool
        table_info = self.client.call_tool("test_get_table_info", {
            "file_path": path, "table_index": 0,
        })
        info = json.loads(table_info)
        self.assertEqual(info["rows"], 3)
        self.assertEqual(info["columns"], 2)

    # -- Search and replace --------------------------------------------------

    def test_search_replace(self):
        path = self._doc_path("test-replace.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        self.client.call_tool("add_paragraph", {
            "file_path": path, "text": "The quick brown fox."
        })
        result = self.client.call_tool("search_replace_text", {
            "file_path": path, "search_text": "fox", "replace_text": "cat",
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("cat", content)
        self.assertNotIn("fox", content)

    # -- Delete paragraph ----------------------------------------------------

    def test_delete_paragraph(self):
        path = self._doc_path("test-delete-para.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        self.client.call_tool("add_paragraph", {
            "file_path": path, "text": "Keep this."
        })
        self.client.call_tool("add_paragraph", {
            "file_path": path, "text": "Delete this."
        })
        result = self.client.call_tool("delete_paragraph", {
            "file_path": path, "paragraph_index": 1,
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertNotIn("Delete this", content)

    # -- Document properties -------------------------------------------------

    def test_get_document_properties(self):
        path = self._doc_path("test-props.docx")
        self.client.call_tool("create_blank_document", {
            "filename": path,
            "title": "My Title",
            "author": "Test Author",
        })
        result = self.client.call_tool("get_document_properties", {
            "file_path": path,
        })
        self.assertIn("My Title", result)

    # -- List documents ------------------------------------------------------

    def test_list_documents(self):
        # Create a couple of files first
        self.client.call_tool("create_blank_document", {
            "filename": self._doc_path("list-a.docx"),
        })
        self.client.call_tool("create_blank_document", {
            "filename": self._doc_path("list-b.odt"),
        })
        result = self.client.call_tool("list_documents", {
            "directory": self.docs_dir,
        })
        self.assertIn("list-a.docx", result)
        self.assertIn("list-b.odt", result)

    # -- Copy document -------------------------------------------------------

    def test_copy_document(self):
        src = self._doc_path("copy-src.docx")
        dst = self._doc_path("copy-dst.docx")
        self.client.call_tool("create_blank_document", {"filename": src})
        self.client.call_tool("add_heading", {
            "file_path": src, "text": "Original", "level": 1
        })
        result = self.client.call_tool("copy_document", {
            "source_path": src, "target_path": dst,
        })
        self.assertNotIn("Error", result)
        self.assertTrue(os.path.exists(dst))
        content = self.client.call_tool("read_text_document", {"file_path": dst})
        self.assertIn("Original", content)

    # -- Insert page break ---------------------------------------------------

    def test_insert_page_break(self):
        path = self._doc_path("test-pagebreak.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        self.client.call_tool("add_paragraph", {
            "file_path": path, "text": "Before break."
        })
        result = self.client.call_tool("insert_page_break", {
            "file_path": path,
        })
        self.assertNotIn("Error", result)
        break_info = self.client.call_tool("test_get_page_break_info", {
            "file_path": path,
        })
        info = json.loads(break_info)
        self.assertGreaterEqual(info["total_page_breaks"], 1)

    # -- Error handling: missing file ----------------------------------------

    def test_read_missing_file_returns_error(self):
        result = self.client.call_tool("read_text_document", {
            "file_path": self._doc_path("does-not-exist.docx"),
        })
        self.assertIn("Error", result)


class TestSlidesTools(unittest.TestCase):
    """End-to-end Slides/Impress tool tests."""

    @classmethod
    def setUpClass(cls):
        cls.tmpdir = tempfile.mkdtemp(prefix="smolpc_mcp_slides_")
        cls.docs_dir = os.path.join(cls.tmpdir, "Documents")
        os.makedirs(cls.docs_dir, exist_ok=True)
        cls.client = McpClient(cls.tmpdir)

    @classmethod
    def tearDownClass(cls):
        cls.client.close()

    def _doc_path(self, filename: str) -> str:
        return os.path.join(self.docs_dir, filename)

    # -- Presentation creation -----------------------------------------------

    def test_create_blank_pptx(self):
        path = self._doc_path("test-create.pptx")
        result = self.client.call_tool("create_blank_presentation", {
            "filename": path,
            "title": "Test Presentation",
        })
        self.assertIn("Successfully created", result)
        self.assertTrue(os.path.exists(path))

    def test_create_blank_odp(self):
        path = self._doc_path("test-create.odp")
        result = self.client.call_tool("create_blank_presentation", {
            "filename": path,
        })
        self.assertIn("Successfully created", result)
        self.assertTrue(os.path.exists(path))

    # -- Read presentation ---------------------------------------------------

    def test_read_presentation(self):
        path = self._doc_path("test-read.pptx")
        self.client.call_tool("create_blank_presentation", {"filename": path})
        self.client.call_tool("add_slide", {
            "file_path": path,
            "title": "Slide One",
            "content": "Hello from test",
            "layout": "Title and Content",
        })
        result = self.client.call_tool("read_presentation", {"file_path": path})
        self.assertIn("Slide One", result)

    # -- Add slide -----------------------------------------------------------

    def test_add_slide(self):
        path = self._doc_path("test-addslide.pptx")
        self.client.call_tool("create_blank_presentation", {"filename": path})
        result = self.client.call_tool("add_slide", {
            "file_path": path,
            "title": "New Slide",
            "content": "Slide body text",
            "layout": "Title and Content",
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_presentation", {"file_path": path})
        self.assertIn("New Slide", content)

    # -- Edit slide title ----------------------------------------------------

    def test_edit_slide_title(self):
        path = self._doc_path("test-edit-title.pptx")
        self.client.call_tool("create_blank_presentation", {"filename": path})
        self.client.call_tool("add_slide", {
            "file_path": path,
            "title": "Old Title",
            "content": "Body",
            "layout": "Title and Content",
        })
        result = self.client.call_tool("edit_slide_title", {
            "file_path": path,
            "slide_index": 0,
            "new_title": "New Title",
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_presentation", {"file_path": path})
        self.assertIn("New Title", content)

    # -- Edit slide content --------------------------------------------------

    def test_edit_slide_content(self):
        path = self._doc_path("test-edit-content.pptx")
        self.client.call_tool("create_blank_presentation", {"filename": path})
        self.client.call_tool("add_slide", {
            "file_path": path,
            "title": "Title",
            "content": "Old Content",
            "layout": "Title and Content",
        })
        result = self.client.call_tool("edit_slide_content", {
            "file_path": path,
            "slide_index": 0,
            "new_content": "Updated Content",
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_presentation", {"file_path": path})
        self.assertIn("Updated Content", content)

    # -- Delete slide --------------------------------------------------------

    def test_delete_slide(self):
        path = self._doc_path("test-delete-slide.pptx")
        self.client.call_tool("create_blank_presentation", {"filename": path})
        self.client.call_tool("add_slide", {
            "file_path": path, "title": "Keep", "content": "A",
            "layout": "Title and Content",
        })
        self.client.call_tool("add_slide", {
            "file_path": path, "title": "Remove", "content": "B",
            "layout": "Title and Content",
        })
        result = self.client.call_tool("delete_slide", {
            "file_path": path, "slide_index": 1,
        })
        self.assertNotIn("Error", result)
        content = self.client.call_tool("read_presentation", {"file_path": path})
        self.assertNotIn("Remove", content)

    # -- Error handling: missing file ----------------------------------------

    def test_read_missing_presentation_returns_error(self):
        result = self.client.call_tool("read_presentation", {
            "file_path": self._doc_path("does-not-exist.pptx"),
        })
        self.assertIn("Error", result)


class TestMultiSessionStability(unittest.TestCase):
    """Test that multiple MCP server sessions can start/stop cleanly."""

    def test_sequential_sessions(self):
        """Start and stop 3 sessions sequentially — no port/process contamination."""
        for i in range(3):
            tmpdir = tempfile.mkdtemp(prefix=f"smolpc_mcp_stability_{i}_")
            docs_dir = os.path.join(tmpdir, "Documents")
            os.makedirs(docs_dir, exist_ok=True)
            client = McpClient(tmpdir)
            try:
                path = os.path.join(docs_dir, f"stability-{i}.docx")
                result = client.call_tool("create_blank_document", {"filename": path})
                self.assertIn("Successfully created", result, f"Session {i} failed")
                self.assertTrue(os.path.exists(path), f"File missing in session {i}")
            finally:
                client.close()


if __name__ == "__main__":
    unittest.main(verbosity=2)
