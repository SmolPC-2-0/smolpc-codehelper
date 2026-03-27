"""
Blender Learning Assistant - HTTP Client Version

Educational addon that helps students learn Blender through:
- Scene-aware Q&A
- Smart suggestions based on current work

Connects to the local Blender Learning Assistant bridge for AI-powered guidance.
"""

import bpy
import os
import traceback
import time

bl_info = {
    "name": "Blender Learning Assistant",
    "author": "SmolPC",
    "version": (7, 2, 0),
    "blender": (3, 0, 0),
    "category": "3D View",
    "description": "Educational assistant for learning Blender (offline)",
}

MAX_SYNC_OBJECTS = 300
MAX_MODIFIERS_PER_OBJECT = 8
MAX_SELECTED_OBJECTS = 32


# ============================================================================
# Scene Data Collection
# ============================================================================

def gather_scene_info():
    """Collect current scene state for educational assistant.

    Each field is gathered individually so that one failure (e.g.
    bpy.context.selected_objects unavailable in a timer callback)
    does not prevent the rest of the scene data from being sent.
    """
    scene_data = {
        'object_count': 0,
        'objects': [],
        'selected_objects': [],
        'active_object': None,
        'mode': 'OBJECT',
        'render_engine': None,
    }

    scene = None
    try:
        scene = bpy.context.scene
    except Exception:
        scene = None

    try:
        if scene is not None:
            scene_data['object_count'] = len(scene.objects)
    except Exception:
        pass

    try:
        scene_data['active_object'] = (
            bpy.context.active_object.name if bpy.context.active_object else None
        )
    except Exception:
        pass

    try:
        selected_objects = []
        for index, obj in enumerate(bpy.context.selected_objects):
            if index >= MAX_SELECTED_OBJECTS:
                break
            selected_objects.append(obj.name)
        scene_data['selected_objects'] = selected_objects
    except Exception:
        pass

    try:
        scene_data['mode'] = bpy.context.mode
    except Exception:
        pass

    try:
        if scene is not None:
            scene_data['render_engine'] = scene.render.engine
    except Exception:
        pass

    try:
        if scene is None:
            return scene_data

        for index, obj in enumerate(scene.objects):
            if index >= MAX_SYNC_OBJECTS:
                break

            modifiers = []
            for modifier_index, modifier in enumerate(obj.modifiers):
                if modifier_index >= MAX_MODIFIERS_PER_OBJECT:
                    break
                modifiers.append({'name': modifier.name, 'type': modifier.type})

            obj_info = {
                'name': obj.name,
                'type': obj.type,
                'modifiers': modifiers,
                'material_count': len(obj.material_slots)
            }
            scene_data['objects'].append(obj_info)
    except Exception as e:
        print(f"Error gathering object details: {e}")

    return scene_data


# ============================================================================
# Bridge Token Auth
# ============================================================================

def _runtime_dir():
    """Return the engine-runtime directory path."""
    local_app_data = os.environ.get('LOCALAPPDATA', '')
    if not local_app_data:
        return None
    return os.path.join(local_app_data, 'SmolPC 2.0', 'engine-runtime')


def _read_bridge_token():
    """Read the bridge auth token written by the Tauri app."""
    runtime_dir = _runtime_dir()
    if not runtime_dir:
        return None
    token_path = os.path.join(runtime_dir, 'bridge-token.txt')
    try:
        with open(token_path, 'r') as f:
            return f.read().strip() or None
    except (OSError, IOError):
        return None


def _read_bridge_port(default=5180):
    """Read the bridge port written by the Tauri app.

    Falls back to *default* when the file is missing or unreadable so
    the addon still works when the bridge uses the preferred port.
    """
    runtime_dir = _runtime_dir()
    if not runtime_dir:
        return default
    port_path = os.path.join(runtime_dir, 'bridge-port.txt')
    try:
        with open(port_path, 'r') as f:
            return int(f.read().strip())
    except (OSError, IOError, ValueError):
        return default


def _bridge_url():
    """Return the base URL for the Blender bridge."""
    port = _read_bridge_port()
    return f"http://127.0.0.1:{port}"


def _auth_headers():
    """Return Authorization header dict if a bridge token is available."""
    token = _read_bridge_token()
    if token:
        return {'Authorization': f'Bearer {token}'}
    return {}


# ============================================================================
# HTTP Client Functions (stdlib only — no 'requests' in Blender's Python)
# ============================================================================

import json as _json
import urllib.request
import urllib.error


def _post_json(url, payload, headers=None, timeout=60):
    """POST JSON and return parsed response. Uses only stdlib."""
    data = _json.dumps(payload).encode('utf-8')
    req = urllib.request.Request(url, data=data, method='POST')
    req.add_header('Content-Type', 'application/json')
    req.add_header('Connection', 'close')
    if headers:
        for key, value in headers.items():
            req.add_header(key, value)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return _json.loads(resp.read().decode('utf-8'))


def _get_json(url, headers=None, timeout=5):
    """GET JSON and return parsed response. Uses only stdlib."""
    req = urllib.request.Request(url, method='GET')
    req.add_header('Connection', 'close')
    if headers:
        for key, value in headers.items():
            req.add_header(key, value)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return _json.loads(resp.read().decode('utf-8'))


def ask_question(question, server_url=None):
    """Ask an educational question through the local assistant bridge."""
    server_url = server_url or _bridge_url()
    try:
        scene_context = gather_scene_info()
        data = _post_json(
            f"{server_url}/ask",
            {'question': question, 'scene_context': scene_context},
            headers=_auth_headers(),
            timeout=60,
        )
        return {
            'answer': data.get('answer', ''),
            'contexts_used': data.get('contexts_used', 0),
            'error': None,
        }
    except urllib.error.URLError:
        return {
            'answer': None,
            'error': 'Cannot connect to Blender Learning Assistant. Open the desktop app and try again.',
        }
    except TimeoutError:
        return {'answer': None, 'error': 'Request timed out.'}
    except Exception as e:
        return {'answer': None, 'error': f'Error: {str(e)}'}


def get_suggestions(server_url=None):
    """Get learning suggestions based on current scene."""
    server_url = server_url or _bridge_url()
    try:
        scene_data = gather_scene_info()
        data = _post_json(
            f"{server_url}/scene_analysis",
            {'goal': 'learning blender', 'scene_data': scene_data},
            headers=_auth_headers(),
            timeout=60,
        )
        return {'suggestions': data.get('suggestions', []), 'error': None}
    except urllib.error.URLError:
        return {'suggestions': None, 'error': 'Cannot connect to Blender Learning Assistant.'}
    except Exception as e:
        return {'suggestions': None, 'error': f'Error: {str(e)}'}


def update_scene_data(server_url=None):
    """Send current scene data to server for caching."""
    server_url = server_url or _bridge_url()
    try:
        scene_data = gather_scene_info()
        _post_json(
            f"{server_url}/scene/update",
            {'scene_data': scene_data},
            headers=_auth_headers(),
            timeout=2,
        )
        return {'success': True, 'error': None}
    except urllib.error.HTTPError as e:
        return {'success': False, 'error': f'HTTP {e.code} from bridge (auth issue?)'}
    except urllib.error.URLError:
        return {'success': False, 'error': 'Server not running'}
    except Exception as e:
        return {'success': False, 'error': str(e)}


def check_server_health(server_url=None, timeout=5):
    """Check if the Blender Learning Assistant bridge is running."""
    server_url = server_url or _bridge_url()
    try:
        data = _get_json(
            f"{server_url}/health",
            headers=_auth_headers(),
            timeout=timeout,
        )
        return {
            'running': True,
            'rag_enabled': data.get('rag_enabled', False),
            'rag_docs': data.get('rag_docs', 0),
            'error': None,
            'last_updated': time.time(),
        }
    except urllib.error.URLError:
        return {'running': False, 'error': 'Server not running', 'last_updated': time.time()}
    except Exception as e:
        return {'running': False, 'error': str(e), 'last_updated': time.time()}


# ============================================================================
# Blender Operators
# ============================================================================

class BLENDERHELPER_OT_ask_question(bpy.types.Operator):
    """Ask a question about Blender"""
    bl_idname = "blenderhelper.ask_question"
    bl_label = "Ask Question"

    def execute(self, context):
        question = context.scene.blenderhelper_question

        if not question.strip():
            self.report({'ERROR'}, "Please enter a question")
            return {'CANCELLED'}

        try:
            self.report({'INFO'}, f"Asking: {question}")

            # Call assistant bridge
            result = ask_question(question)

            if result['error']:
                self.report({'ERROR'}, result['error'])
                return {'CANCELLED'}

            answer = result['answer']

            if not answer:
                self.report({'ERROR'}, "No answer received")
                return {'CANCELLED'}

            # Store answer
            context.scene.blenderhelper_answer = answer

            # Show in console
            contexts = result.get('contexts_used', 0)
            print("\n" + "="*60)
            print(f"Question: {question}")
            print("="*60)
            print(f"Answer ({contexts} docs used):\n")
            print(answer)
            print("="*60 + "\n")

            self.report({'INFO'}, "Answer ready. Check the panel or console.")
            return {'FINISHED'}

        except Exception as e:
            error_msg = str(e)
            self.report({'ERROR'}, f"Failed: {error_msg}")
            print(f"[ERROR] {error_msg}")
            traceback.print_exc()
            return {'CANCELLED'}


class BLENDERHELPER_OT_get_suggestions(bpy.types.Operator):
    """Get learning suggestions based on current scene"""
    bl_idname = "blenderhelper.get_suggestions"
    bl_label = "Get Suggestions"

    def execute(self, context):
        try:
            self.report({'INFO'}, "Analyzing your scene...")

            # Get suggestions
            result = get_suggestions()

            if result['error']:
                self.report({'ERROR'}, result['error'])
                return {'CANCELLED'}

            suggestions = result['suggestions']

            if not suggestions:
                self.report({'ERROR'}, "No suggestions received")
                return {'CANCELLED'}

            if isinstance(suggestions, list):
                suggestions_text = "\n".join(suggestions)
            else:
                suggestions_text = str(suggestions)

            # Store suggestions
            context.scene.blenderhelper_suggestions = suggestions_text

            # Show in console
            print("\n" + "="*60)
            print("Learning Suggestions:")
            print("="*60)
            print(suggestions_text)
            print("="*60 + "\n")

            self.report({'INFO'}, "Got suggestions. Check the panel.")
            return {'FINISHED'}

        except Exception as e:
            error_msg = str(e)
            self.report({'ERROR'}, f"Failed: {error_msg}")
            print(f"[ERROR] {error_msg}")
            traceback.print_exc()
            return {'CANCELLED'}


class BLENDERHELPER_OT_show_answer(bpy.types.Operator):
    """Show answer in a popup"""
    bl_idname = "blenderhelper.show_answer"
    bl_label = "View Answer"

    def execute(self, context):
        return {'FINISHED'}

    def invoke(self, context, event):
        return context.window_manager.invoke_props_dialog(self, width=600)

    def draw(self, context):
        layout = self.layout
        answer = context.scene.blenderhelper_answer

        if not answer:
            layout.label(text="No answer yet. Ask a question first!")
            return

        # Word wrap the answer
        box = layout.box()
        max_chars_per_line = 70
        words = answer.split()
        current_line = ""

        for word in words:
            if len(current_line) + len(word) + 1 <= max_chars_per_line:
                current_line += word + " "
            else:
                if current_line:
                    box.label(text=current_line.strip())
                current_line = word + " "

        if current_line:
            box.label(text=current_line.strip())


class BLENDERHELPER_OT_test_server(bpy.types.Operator):
    """Test connection to Blender Learning Assistant"""
    bl_idname = "blenderhelper.test_server"
    bl_label = "Test Server"

    def execute(self, context):
        health = check_server_health()

        if health['running']:
            if health['rag_enabled']:
                msg = f"Bridge running. RAG: {health['rag_docs']} docs"
            else:
                msg = "Bridge running (RAG disabled)"
            self.report({'INFO'}, msg)
        else:
            self.report({'ERROR'}, f"Bridge not running: {health['error']}")
            print("\nOpen Blender Learning Assistant desktop app, then retry.")

        return {'FINISHED'}


# ============================================================================
# UI Panel
# ============================================================================

class BLENDERHELPER_PT_panel(bpy.types.Panel):
    """Main panel for Blender Learning Assistant"""
    bl_label = "Learning Assistant"
    bl_idname = "BLENDERHELPER_PT_panel"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'Learn'

    def draw(self, context):
        layout = self.layout
        scene = context.scene

        # Scene Overview
        box = layout.box()
        box.label(text="Current Scene:", icon='SCENE_DATA')

        scene_info = gather_scene_info()
        box.label(text=f"Objects: {scene_info.get('object_count', 0)}")

        active = scene_info.get('active_object')
        if active:
            box.label(text=f"Active: {active}")

        box.label(text=f"Mode: {scene_info.get('mode', 'OBJECT')}")

        # Ask Question Section
        layout.separator()
        box = layout.box()
        box.label(text="Ask a Question:", icon='QUESTION')
        box.prop(scene, "blenderhelper_question", text="")

        row = box.row(align=True)
        row.scale_y = 1.3
        row.operator("blenderhelper.ask_question", text="Ask", icon='VIEWZOOM')
        row.operator("blenderhelper.show_answer", text="View Answer", icon='TEXT')

        # Suggestions Section
        layout.separator()
        box = layout.box()
        box.label(text="What to Try Next:", icon='LIGHT')

        row = box.row()
        row.scale_y = 1.3
        row.operator("blenderhelper.get_suggestions", text="Get Suggestions", icon='LIGHTPROBE_GRID')

        # Show suggestions if available
        suggestions = scene.blenderhelper_suggestions
        if suggestions:
            suggestion_box = box.box()
            # Show first few lines
            lines = suggestions.split('\n')[:5]
            for line in lines:
                if line.strip():
                    suggestion_box.label(text=line[:60])
            if len(lines) > 5:
                suggestion_box.label(text="... (see console for more)")

        # Server Status
        layout.separator()
        box = layout.box()
        box.label(text="System Status:", icon='SETTINGS')

        # Read cached server health (non-blocking - safe for UI thread)
        health = get_cached_server_health()

        if health['running']:
            box.label(text="Bridge: Running", icon='CHECKMARK')

            if health['rag_enabled']:
                box.label(text=f"   Docs: {health['rag_docs']}")
        else:
            box.label(text="Bridge: Not running", icon='ERROR')
            box.label(text="   Open desktop app")

        # Test button
        row = box.row()
        row.operator("blenderhelper.test_server", text="Test Connection", icon='PLUGIN')


# ============================================================================
# Scene Data Sync Timer
# ============================================================================

# Global cache for server health status (updated by timer, read by UI)
_server_health_cache = {
    'running': False,
    'rag_enabled': False,
    'rag_docs': 0,
    'error': 'Not checked yet',
    'last_updated': 0
}


def sync_scene_timer():
    """Periodic timer to send scene data to server and update health cache."""
    global _server_health_cache

    try:
        # Update health cache
        health = check_server_health(timeout=0.5)
        _server_health_cache = {**_server_health_cache, **health}

        # Always attempt sync so a flaky health probe does not block updates.
        result = update_scene_data()
        if result['success']:
            _server_health_cache = {
                **_server_health_cache,
                'running': True,
                'error': None,
                'last_updated': time.time(),
            }
        elif result.get('error') and result['error'] != 'Server not running':
            print(f"[BlenderHelper] Scene sync failed: {result['error']}")
    except Exception:
        # Never let an exception escape — Blender silently kills the timer.
        pass

    # Re-register timer (every 5 seconds)
    return 5.0


def get_cached_server_health():
    """Get cached server health status (non-blocking, safe for UI thread)."""
    global _server_health_cache
    return _server_health_cache.copy()


# ============================================================================
# Registration
# ============================================================================

classes = [
    BLENDERHELPER_OT_ask_question,
    BLENDERHELPER_OT_get_suggestions,
    BLENDERHELPER_OT_show_answer,
    BLENDERHELPER_OT_test_server,
    BLENDERHELPER_PT_panel,
]


def register():
    # Register properties
    bpy.types.Scene.blenderhelper_question = bpy.props.StringProperty(
        name="Question",
        description="Ask a question about Blender",
        default="What is a modifier?"
    )

    bpy.types.Scene.blenderhelper_answer = bpy.props.StringProperty(
        name="Answer",
        description="Last answer from learning assistant",
        default=""
    )

    bpy.types.Scene.blenderhelper_suggestions = bpy.props.StringProperty(
        name="Suggestions",
        description="Learning suggestions based on current scene",
        default=""
    )

    # Register classes
    for cls in classes:
        bpy.utils.register_class(cls)

    # Start scene data sync timer
    if not bpy.app.timers.is_registered(sync_scene_timer):
        bpy.app.timers.register(sync_scene_timer, first_interval=2.0)

    print("\n" + "="*60)
    print("Blender Learning Assistant loaded successfully!")
    print("="*60)
    print("Start the Blender Learning Assistant desktop app")
    print("Find addon in: 3D Viewport > Sidebar (N) > Learn")
    print("Scene sync: Active (updates every 5 seconds)")
    print("="*60 + "\n")


def unregister():
    # Stop scene sync timer
    if bpy.app.timers.is_registered(sync_scene_timer):
        bpy.app.timers.unregister(sync_scene_timer)

    # Unregister classes
    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)

    # Remove properties
    del bpy.types.Scene.blenderhelper_question
    del bpy.types.Scene.blenderhelper_answer
    del bpy.types.Scene.blenderhelper_suggestions


if __name__ == "__main__":
    register()
