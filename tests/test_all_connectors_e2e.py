#!/usr/bin/env python3
"""
End-to-end connector tests: send prompts, verify tool calls produce correct responses.

Tests each connector's pipeline as far as possible without requiring host apps:
- LibreOffice: Full pipeline (MCP server over stdio) -- create docs, manipulate, verify
- Blender: RAG retrieval (keyword search over bundled Blender API docs)
- GIMP: Bridge spawn test (verify MCP bridge script exists and is syntactically valid)

Usage:
    python tests/test_all_connectors_e2e.py
"""
import json
import os
import py_compile
import shutil
import tempfile
import unittest
from pathlib import Path

from mcp_test_client import McpClient

REPO_ROOT = Path(__file__).resolve().parent.parent
MCP_SCRIPT = str(REPO_ROOT / "connectors" / "libreoffice" / "resources" / "mcp_server" / "mcp_server.py")


def _assert_success(test_case, result: str, context: str = ""):
    """Assert the tool result starts with a success indicator, not 'Error:'."""
    test_case.assertFalse(
        result.startswith("Error"),
        f"Tool returned an error{' (' + context + ')' if context else ''}: {result[:200]}",
    )


# =============================================================================
# LibreOffice: Full MCP tool call tests
# =============================================================================

class TestLibreOfficeWriterPrompts(unittest.TestCase):
    """Simulate user prompts by calling the tools a planner would select."""

    @classmethod
    def setUpClass(cls):
        cls.tmpdir = tempfile.mkdtemp(prefix="e2e_writer_")
        cls.docs = os.path.join(cls.tmpdir, "Documents")
        os.makedirs(cls.docs, exist_ok=True)
        cls.client = McpClient(MCP_SCRIPT, cls.tmpdir)

    @classmethod
    def tearDownClass(cls):
        cls.client.close()
        shutil.rmtree(cls.tmpdir, ignore_errors=True)

    def _path(self, name):
        return os.path.join(self.docs, name)

    def test_prompt_create_blank_document(self):
        """Prompt: 'Create a blank document called lesson-plan.odt'"""
        path = self._path("lesson-plan.odt")
        result = self.client.call_tool("create_blank_document", {"filename": path})
        self.assertIn("Successfully created", result)
        self.assertTrue(os.path.exists(path))

    def test_prompt_create_docx_and_add_heading(self):
        """Prompt: 'Create a document called notes.docx and add a heading Introduction'"""
        path = self._path("notes.docx")
        r1 = self.client.call_tool("create_blank_document", {"filename": path})
        self.assertIn("Successfully created", r1)
        r2 = self.client.call_tool("add_heading", {"file_path": path, "text": "Introduction", "level": 1})
        _assert_success(self, r2, "add_heading")
        content = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("Introduction", content)

    def test_prompt_add_table_with_data(self):
        """Prompt: 'Add a 3x2 table with headers Name and Score'"""
        path = self._path("table-test.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        result = self.client.call_tool("add_table", {
            "file_path": path, "rows": 3, "columns": 2,
            "data": [["Name", "Score"], ["Alice", "95"], ["Bob", "87"]],
        })
        _assert_success(self, result, "add_table")
        info = json.loads(self.client.call_tool("test_get_table_info", {"file_path": path, "table_index": 0}))
        self.assertEqual(info["rows"], 3)
        self.assertEqual(info["columns"], 2)
        self.assertIn("Alice", str(info["data"]))

    def test_prompt_search_and_replace(self):
        """Prompt: 'Replace all occurrences of dog with cat in my document'"""
        path = self._path("replace-test.docx")
        self.client.call_tool("create_blank_document", {"filename": path})
        self.client.call_tool("add_paragraph", {"file_path": path, "text": "The dog ran. Another dog barked."})
        result = self.client.call_tool("search_replace_text", {
            "file_path": path, "search_text": "dog", "replace_text": "cat",
        })
        _assert_success(self, result, "search_replace_text")
        content = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertNotIn("dog", content)
        self.assertIn("cat", content)

    def test_prompt_multi_step_document(self):
        """Prompt: 'Create a lesson plan with heading, 2 paragraphs, and a page break'"""
        path = self._path("multi-step.docx")
        self.client.call_tool("create_blank_document", {"filename": path, "title": "Lesson Plan"})
        self.client.call_tool("add_heading", {"file_path": path, "text": "Lesson Plan", "level": 1})
        self.client.call_tool("add_paragraph", {"file_path": path, "text": "Objective: Learn about AI in schools."})
        self.client.call_tool("add_paragraph", {"file_path": path, "text": "Materials: Laptop, SmolPC software."})
        self.client.call_tool("insert_page_break", {"file_path": path})
        self.client.call_tool("add_heading", {"file_path": path, "text": "Activities", "level": 2})

        content = self.client.call_tool("read_text_document", {"file_path": path})
        self.assertIn("Lesson Plan", content)
        self.assertIn("Objective", content)
        self.assertIn("Materials", content)

        breaks = json.loads(self.client.call_tool("test_get_page_break_info", {"file_path": path}))
        self.assertGreaterEqual(breaks["total_page_breaks"], 1)


class TestLibreOfficeSlidesPrompts(unittest.TestCase):
    """Simulate user prompts for Slides/Impress mode."""

    @classmethod
    def setUpClass(cls):
        cls.tmpdir = tempfile.mkdtemp(prefix="e2e_slides_")
        cls.docs = os.path.join(cls.tmpdir, "Documents")
        os.makedirs(cls.docs, exist_ok=True)
        cls.client = McpClient(MCP_SCRIPT, cls.tmpdir)

    @classmethod
    def tearDownClass(cls):
        cls.client.close()
        shutil.rmtree(cls.tmpdir, ignore_errors=True)

    def _path(self, name):
        return os.path.join(self.docs, name)

    def test_prompt_create_blank_presentation(self):
        """Prompt: 'Create a blank presentation called demo-pitch.odp'"""
        path = self._path("demo-pitch.odp")
        result = self.client.call_tool("create_blank_presentation", {"filename": path})
        self.assertIn("Successfully created", result)
        self.assertTrue(os.path.exists(path))

    def test_prompt_create_pptx_with_slides(self):
        """Prompt: 'Create a presentation with a title slide and a content slide'"""
        path = self._path("two-slides.pptx")
        self.client.call_tool("create_blank_presentation", {"filename": path})
        self.client.call_tool("add_slide", {
            "file_path": path, "title": "Welcome to SmolPC",
            "content": "Offline AI for schools", "layout": "Title and Content",
        })
        self.client.call_tool("add_slide", {
            "file_path": path, "title": "Features",
            "content": "Privacy-first, budget hardware, Intel NPU", "layout": "Title and Content",
        })
        content = self.client.call_tool("read_presentation", {"file_path": path})
        self.assertIn("Welcome to SmolPC", content)
        self.assertIn("Features", content)

    def test_prompt_edit_slide_title(self):
        """Prompt: 'Change the title of slide 1 to Updated Title'"""
        path = self._path("edit-title.pptx")
        self.client.call_tool("create_blank_presentation", {"filename": path})
        self.client.call_tool("add_slide", {
            "file_path": path, "title": "Old Title",
            "content": "Body", "layout": "Title and Content",
        })
        result = self.client.call_tool("edit_slide_title", {
            "file_path": path, "slide_index": 0, "new_title": "Updated Title",
        })
        _assert_success(self, result, "edit_slide_title")
        content = self.client.call_tool("read_presentation", {"file_path": path})
        self.assertIn("Updated Title", content)

    def test_prompt_delete_slide(self):
        """Prompt: 'Delete the second slide from my presentation'"""
        path = self._path("delete-slide.pptx")
        self.client.call_tool("create_blank_presentation", {"filename": path})
        self.client.call_tool("add_slide", {
            "file_path": path, "title": "Keep", "content": "A", "layout": "Title and Content",
        })
        self.client.call_tool("add_slide", {
            "file_path": path, "title": "Remove This", "content": "B", "layout": "Title and Content",
        })
        result = self.client.call_tool("delete_slide", {"file_path": path, "slide_index": 1})
        _assert_success(self, result, "delete_slide")
        content = self.client.call_tool("read_presentation", {"file_path": path})
        self.assertNotIn("Remove This", content)
        self.assertIn("Keep", content)


# =============================================================================
# Blender: RAG retrieval tests (no running Blender needed)
# =============================================================================

class TestBlenderRagRetrieval(unittest.TestCase):
    """Test Blender RAG context retrieval using the bundled metadata."""

    RAG_METADATA = REPO_ROOT / "connectors" / "blender" / "resources" / "rag_system" / "simple_db" / "metadata.json"

    @classmethod
    def setUpClass(cls):
        if not cls.RAG_METADATA.exists():
            raise unittest.SkipTest(f"Blender RAG metadata not found at {cls.RAG_METADATA}")
        with open(cls.RAG_METADATA, encoding="utf-8") as f:
            cls.metadata = json.load(f)

    def test_metadata_loads_and_has_entries(self):
        """RAG metadata file loads and contains Blender API docs."""
        self.assertIsInstance(self.metadata, list)
        self.assertGreater(len(self.metadata), 10, "Should have many Blender API entries")

    def test_entries_have_required_fields(self):
        """Each entry has text, signature, and url fields."""
        for entry in self.metadata[:20]:
            self.assertIn("text", entry, f"Entry missing 'text': {entry}")
            self.assertIn("signature", entry, f"Entry missing 'signature': {entry}")
            self.assertIn("url", entry, f"Entry missing 'url': {entry}")

    def test_query_bevel_modifier_finds_relevant_results(self):
        """Prompt: 'How do I add a bevel modifier?' should match bevel entries."""
        query_terms = {"bevel", "modifier"}
        matches = [
            e for e in self.metadata
            if any(t in e["text"].lower() or t in e["signature"].lower() for t in query_terms)
        ]
        self.assertGreater(len(matches), 0, "Should find bevel modifier docs")
        self.assertTrue(
            any("bevel" in m["signature"].lower() for m in matches),
            "At least one match should reference bevel in signature",
        )

    def test_query_mesh_operations_finds_results(self):
        """Prompt: 'How do I subdivide a mesh?' should match mesh operations."""
        query_terms = {"subdivide", "mesh"}
        matches = [
            e for e in self.metadata
            if any(t in e["text"].lower() or t in e["signature"].lower() for t in query_terms)
        ]
        self.assertGreater(len(matches), 0, "Should find mesh subdivision docs")

    def test_query_render_settings_finds_results(self):
        """Prompt: 'How do I change render settings?' should match render entries."""
        matches = [
            e for e in self.metadata
            if "render" in e["text"].lower() or "render" in e["signature"].lower()
        ]
        self.assertGreater(len(matches), 0, "Should find render-related docs")

    def test_query_keyframe_animation_finds_results(self):
        """Prompt: 'How do I add keyframes for animation?'"""
        query_terms = {"keyframe", "animation"}
        matches = [
            e for e in self.metadata
            if any(t in e["text"].lower() or t in e["signature"].lower() for t in query_terms)
        ]
        self.assertGreater(len(matches), 0, "Should find animation/keyframe docs")


# =============================================================================
# Blender: Addon script validation
# =============================================================================

class TestBlenderAddon(unittest.TestCase):
    """Validate the Blender addon script is syntactically correct."""

    ADDON_PATH = REPO_ROOT / "connectors" / "blender" / "resources" / "addon" / "blender_helper_http.py"

    def test_addon_exists(self):
        self.assertTrue(self.ADDON_PATH.exists(), f"Addon not found at {self.ADDON_PATH}")

    def test_addon_compiles(self):
        """Addon Python file should be syntactically valid."""
        try:
            py_compile.compile(str(self.ADDON_PATH), doraise=True)
        except py_compile.PyCompileError as e:
            self.fail(f"Blender addon has syntax error: {e}")

    def test_addon_has_bl_info(self):
        """Addon must define bl_info for Blender to recognize it."""
        content = self.ADDON_PATH.read_text()
        self.assertIn("bl_info", content)

    def test_addon_has_register_and_unregister(self):
        """Addon must define register() and unregister() functions."""
        content = self.ADDON_PATH.read_text()
        self.assertIn("def register()", content)
        self.assertIn("def unregister()", content)

    def test_addon_has_scene_sync(self):
        """Addon should sync scene data to the bridge."""
        content = self.ADDON_PATH.read_text()
        self.assertIn("sync_scene_timer", content)
        self.assertIn("update_scene_data", content)


# =============================================================================
# GIMP: Bridge and plugin script validation
# =============================================================================

class TestGimpConnectorResources(unittest.TestCase):
    """Validate GIMP connector Python resources exist and compile."""

    GIMP_RESOURCES = REPO_ROOT / "connectors" / "gimp" / "resources"

    def test_bridge_script_exists(self):
        bridge = self.GIMP_RESOURCES / "bridge" / "smolpc_gimp_mcp_tcp_bridge.py"
        self.assertTrue(bridge.exists(), f"GIMP bridge script not found at {bridge}")

    def test_bridge_script_compiles(self):
        bridge = self.GIMP_RESOURCES / "bridge" / "smolpc_gimp_mcp_tcp_bridge.py"
        if not bridge.exists():
            self.skipTest("Bridge script not found")
        try:
            py_compile.compile(str(bridge), doraise=True)
        except py_compile.PyCompileError as e:
            self.fail(f"GIMP bridge script has syntax error: {e}")

    def test_plugin_script_exists(self):
        plugin = self.GIMP_RESOURCES / "plugin" / "gimp-mcp-plugin" / "gimp-mcp-plugin.py"
        self.assertTrue(plugin.exists(), f"GIMP plugin script not found at {plugin}")

    def test_plugin_script_compiles(self):
        plugin = self.GIMP_RESOURCES / "plugin" / "gimp-mcp-plugin" / "gimp-mcp-plugin.py"
        if not plugin.exists():
            self.skipTest("Plugin script not found")
        try:
            py_compile.compile(str(plugin), doraise=True)
        except py_compile.PyCompileError as e:
            self.fail(f"GIMP plugin script has syntax error: {e}")

    def test_upstream_mcp_server_exists(self):
        server = self.GIMP_RESOURCES / "upstream" / "gimp_mcp_server.py"
        self.assertTrue(server.exists(), f"GIMP upstream MCP server not found at {server}")

    def test_upstream_mcp_server_compiles(self):
        server = self.GIMP_RESOURCES / "upstream" / "gimp_mcp_server.py"
        if not server.exists():
            self.skipTest("Upstream MCP server not found")
        try:
            py_compile.compile(str(server), doraise=True)
        except py_compile.PyCompileError as e:
            self.fail(f"GIMP upstream MCP server has syntax error: {e}")


# =============================================================================
# LibreOffice: MCP server resource validation
# =============================================================================

class TestLibreOfficeMcpServerResources(unittest.TestCase):
    """Validate LibreOffice MCP server resources."""

    MCP_DIR = REPO_ROOT / "connectors" / "libreoffice" / "resources" / "mcp_server"

    def test_mcp_server_exists(self):
        self.assertTrue((self.MCP_DIR / "mcp_server.py").exists())

    def test_mcp_server_compiles(self):
        try:
            py_compile.compile(str(self.MCP_DIR / "mcp_server.py"), doraise=True)
        except py_compile.PyCompileError as e:
            self.fail(f"MCP server has syntax error: {e}")

    def test_test_functions_exists(self):
        self.assertTrue((self.MCP_DIR / "test_functions.py").exists())

    def test_test_functions_compiles(self):
        try:
            py_compile.compile(str(self.MCP_DIR / "test_functions.py"), doraise=True)
        except py_compile.PyCompileError as e:
            self.fail(f"test_functions has syntax error: {e}")


if __name__ == "__main__":
    unittest.main(verbosity=2)
