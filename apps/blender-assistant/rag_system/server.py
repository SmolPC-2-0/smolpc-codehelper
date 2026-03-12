"""
RAG HTTP Server for Blender Helper AI

This server runs in system Python (outside Blender) and handles:
- RAG (vector search with ChromaDB or simple NumPy)
- Ollama LLM queries
- Returns generated code to Blender via HTTP

Runs completely offline on localhost:5179
"""

from flask import Flask, request, jsonify
from flask_cors import CORS
import os
import sys
from pathlib import Path
import traceback
import json
import re
import time
import numpy as np

# Try to import RAG dependencies
try:
    from sentence_transformers import SentenceTransformer
    HAS_TRANSFORMERS = True
except ImportError:
    print("[RAG] Warning: sentence-transformers not found, RAG will be disabled")
    HAS_TRANSFORMERS = False

try:
    import requests as req_lib
    HAS_REQUESTS = True
except ImportError:
    print("[Dependencies] Error: requests library required")
    sys.exit(1)


app = Flask(__name__)
# CORS restricted to localhost origins only for security
CORS(app, origins=[
    'http://127.0.0.1:*',
    'http://localhost:*',
    'tauri://localhost'
])

# Configuration
RAG_DIR = Path(__file__).parent
DB_PATH = RAG_DIR / "simple_db"


class RAGSystem:
    """Simple RAG using NumPy arrays."""

    def __init__(self):
        self.initialized = False
        self.embeddings = None
        self.metadata = None
        self.embedding_model = None

    def initialize(self):
        """Load the RAG database."""
        if self.initialized:
            return True

        if not HAS_TRANSFORMERS:
            print("[RAG] Warning: RAG disabled - sentence-transformers not installed")
            return False

        try:
            print("[RAG] Info: Loading RAG system...")

            # Load embedding model
            self.embedding_model = SentenceTransformer('all-MiniLM-L6-v2')

            # Load database
            embeddings_file = DB_PATH / "embeddings.npy"
            metadata_json_file = DB_PATH / "metadata.json"
            metadata_pickle_file = DB_PATH / "metadata.pkl"

            if not embeddings_file.exists():
                print(f"[RAG] Warning: Database not found at {DB_PATH}")
                print("[RAG] Info: Run indexer_simple.py first to build the knowledge base")
                return False
            if not metadata_json_file.exists() and not metadata_pickle_file.exists():
                print(f"[RAG] Warning: metadata.json not found at {metadata_json_file}")
                print("[RAG] Info: Rebuild the knowledge base to generate metadata.json")
                return False

            self.embeddings = np.load(embeddings_file)

            if metadata_json_file.exists():
                with open(metadata_json_file, 'r', encoding='utf-8') as f:
                    self.metadata = json.load(f)
            else:
                # Pickle is intentionally disabled by default because loading untrusted
                # pickle data can execute arbitrary code.
                allow_pickle = os.getenv("BLENDER_HELPER_ALLOW_PICKLE_METADATA", "0") == "1"
                if not allow_pickle:
                    print("[RAG] Error: metadata.json missing and unsafe pickle fallback disabled")
                    print("[RAG] Error: Rebuild database or set BLENDER_HELPER_ALLOW_PICKLE_METADATA=1")
                    return False

                import pickle
                with open(metadata_pickle_file, 'rb') as f:
                    self.metadata = pickle.load(f)
                print("[RAG] Warning: Loaded metadata.pkl fallback (unsafe). Prefer metadata.json.")

            self.initialized = True
            print(f"[RAG] OK: Successfully loaded {len(self.metadata)} documents")
            return True

        except Exception as e:
            print(f"[RAG] Error: Initialization failed - {e}")
            traceback.print_exc()
            return False

    def retrieve_context(self, query, n_results=3):
        """Retrieve relevant documentation."""
        if not self.initialize():
            return []

        try:
            # Embed query
            query_embedding = self.embedding_model.encode([query])[0]

            # Calculate cosine similarity with division by zero protection
            norms = np.linalg.norm(self.embeddings, axis=1)
            query_norm = np.linalg.norm(query_embedding)

            # Avoid division by zero
            norms = np.where(norms == 0, 1, norms)
            if query_norm == 0:
                query_norm = 1

            similarities = np.dot(self.embeddings, query_embedding) / (norms * query_norm)

            # Get top N
            top_indices = np.argsort(similarities)[-n_results:][::-1]

            # Format results
            contexts = []
            for idx in top_indices:
                chunk = self.metadata[idx]
                contexts.append({
                    'text': chunk['text'],
                    'signature': chunk['signature'],
                    'url': chunk['url'],
                    'similarity': float(similarities[idx])
                })

            return contexts

        except Exception as e:
            print(f"[RAG] Error: Context retrieval failed - {e}")
            traceback.print_exc()
            return []


# Global RAG instance
rag = RAGSystem()

# Global scene data cache (last received from Blender)
cached_scene_data = {
    'scene_data': None,
    'last_update': None
}


def call_ollama(system_prompt, user_prompt, model=None, temperature=0.7, timeout=120):
    """
    Call local Ollama API.

    Args:
        system_prompt: System instructions for the LLM
        user_prompt: User's actual question/request
        model: Model name (default from env or qwen2.5:7b-instruct-q4_K_M)
        temperature: Sampling temperature for creativity (0.0-1.0)
        timeout: Request timeout in seconds (default 120)

    Note: 120-second timeout is needed because:
    - First request loads the model into memory (~10-30 seconds)
    - Large context windows with RAG data may take time to process
    - Complex educational responses require reasoning time
    - Better to have a long timeout than fail on legitimate requests
    """
    if model is None:
        model = os.getenv("OLLAMA_MODEL", "qwen2.5:7b-instruct-q4_K_M")

    payload = {
        "model": model,
        "stream": False,
        "options": {"temperature": temperature},
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ]
    }

    try:
        response = req_lib.post(
            "http://127.0.0.1:11434/api/chat",
            json=payload,
            timeout=timeout
        )
        response.raise_for_status()
        data = response.json()
        return data["message"]["content"]
    except req_lib.exceptions.ConnectionError:
        raise Exception("Ollama not running. Start it with: ollama serve")
    except Exception as e:
        raise Exception(f"Ollama request failed: {e}")


def validate_model_name(model):
    """Validate optional model parameter before passing to Ollama."""
    if model is None:
        return None, None

    if not isinstance(model, str):
        return None, "Model must be a string"

    model = model.strip()
    if not model:
        return None, "Model must be a non-empty string"
    if len(model) > 128:
        return None, "Model name too long (max 128 chars)"
    if not re.match(r"^[A-Za-z0-9._:/-]+$", model):
        return None, "Model contains invalid characters"

    return model, None


@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint."""
    return jsonify({
        'status': 'ok',
        'rag_enabled': rag.initialized,
        'rag_docs': len(rag.metadata) if rag.initialized else 0
    })


@app.route('/rag/retrieve', methods=['POST'])
def retrieve_rag():
    """Retrieve RAG context only (no Ollama call)."""
    try:
        data = request.json
        if data is None:
            return jsonify({'error': 'Invalid JSON or Content-Type must be application/json'}), 400

        query = data.get('query', '')
        if not isinstance(query, str):
            return jsonify({'error': 'Query must be a string'}), 400

        query = query.strip()
        if not query:
            return jsonify({'error': 'Query must be a non-empty string'}), 400

        if len(query) > 10000:
            return jsonify({'error': 'Query too long (max 10,000 characters)'}), 400

        n_results = data.get('n_results', 3)
        if not isinstance(n_results, int):
            return jsonify({'error': 'n_results must be an integer'}), 400
        if n_results < 1 or n_results > 10:
            return jsonify({'error': 'n_results must be between 1 and 10'}), 400

        contexts = rag.retrieve_context(query, n_results=n_results)

        return jsonify({
            'contexts': contexts,
            'rag_enabled': rag.initialized
        })
    except Exception as e:
        print(f"[RAG] Error: Failed to retrieve context - {e}")
        traceback.print_exc()
        return jsonify({'error': str(e)}), 500


@app.route('/scene/update', methods=['POST'])
def update_scene():
    """Receive scene data from Blender addon and cache it."""
    try:
        # Validate input
        data = request.json
        if data is None:
            return jsonify({'error': 'Invalid JSON or Content-Type must be application/json'}), 400

        scene_data = data.get('scene_data', {})

        # Validate scene_data structure
        if not isinstance(scene_data, dict):
            return jsonify({'error': 'Scene data must be an object'}), 400

        # Enforce reasonable size limits to prevent memory issues
        # Max ~1MB of JSON data (typical scene is 1-10KB)
        scene_json_size = len(json.dumps(scene_data))
        if scene_json_size > 1_000_000:  # 1 MB limit
            return jsonify({'error': 'Scene data too large (max 1MB)'}), 400

        # Validate key fields if present
        if 'object_count' in scene_data:
            if not isinstance(scene_data['object_count'], int):
                return jsonify({'error': 'object_count must be an integer'}), 400
            if scene_data['object_count'] < 0 or scene_data['object_count'] > 100000:
                return jsonify({'error': 'Invalid object_count (must be 0-100000)'}), 400

        if 'active_object' in scene_data:
            if not isinstance(scene_data['active_object'], (str, type(None))):
                return jsonify({'error': 'active_object must be a string or null'}), 400
            if isinstance(scene_data['active_object'], str) and len(scene_data['active_object']) > 1000:
                return jsonify({'error': 'active_object name too long (max 1000 chars)'}), 400

        if 'mode' in scene_data:
            if not isinstance(scene_data['mode'], str):
                return jsonify({'error': 'mode must be a string'}), 400
            if len(scene_data['mode']) > 100:
                return jsonify({'error': 'mode name too long (max 100 chars)'}), 400

        if 'objects' in scene_data:
            if not isinstance(scene_data['objects'], list):
                return jsonify({'error': 'objects must be an array'}), 400
            if len(scene_data['objects']) > 100000:
                return jsonify({'error': 'Too many objects (max 100000)'}), 400

            # Validate each object in the list
            for i, obj in enumerate(scene_data['objects']):
                if not isinstance(obj, dict):
                    return jsonify({'error': f'objects[{i}] must be an object'}), 400
                if 'name' in obj and not isinstance(obj['name'], str):
                    return jsonify({'error': f'objects[{i}].name must be a string'}), 400
                if 'type' in obj and not isinstance(obj['type'], str):
                    return jsonify({'error': f'objects[{i}].type must be a string'}), 400
                if 'modifiers' in obj and not isinstance(obj['modifiers'], list):
                    return jsonify({'error': f'objects[{i}].modifiers must be an array'}), 400

        # Update cache
        cached_scene_data['scene_data'] = scene_data
        cached_scene_data['last_update'] = time.time()

        return jsonify({'status': 'ok', 'message': 'Scene data updated'})

    except Exception as e:
        print(f"[Scene] Error: Failed to update scene data - {e}")
        return jsonify({'error': str(e)}), 500


@app.route('/scene/current', methods=['GET'])
def get_current_scene():
    """Get the cached scene data (for frontend)."""
    try:
        if cached_scene_data['scene_data'] is None:
            return jsonify({
                'connected': False,
                'message': 'No scene data available. Make sure Blender addon is installed and active.'
            })

        age = time.time() - cached_scene_data['last_update']

        # Consider stale if older than 30 seconds
        if age > 30:
            return jsonify({
                'connected': False,
                'message': 'Scene data is stale. Blender may not be connected.',
                'last_update': cached_scene_data['last_update']
            })

        return jsonify({
            'connected': True,
            'scene_data': cached_scene_data['scene_data'],
            'last_update': cached_scene_data['last_update']
        })

    except Exception as e:
        print(f"[Scene] Error: Failed to get scene data - {e}")
        return jsonify({'error': str(e)}), 500


@app.route('/ask', methods=['POST'])
def ask_question():
    """Answer educational questions about Blender."""
    try:
        # Validate input
        data = request.json
        if data is None:
            return jsonify({'error': 'Invalid JSON or Content-Type must be application/json'}), 400

        question = data.get('question', '')
        scene_context = data.get('scene_context', {})
        model, model_error = validate_model_name(data.get('model'))
        if model_error:
            return jsonify({'error': model_error}), 400

        # Validate question
        if not isinstance(question, str):
            return jsonify({'error': 'Question must be a string'}), 400

        question = question.strip()

        # Enforce input length limits (10,000 chars = ~2,500 words)
        if len(question) > 10000:
            return jsonify({'error': 'Question too long (max 10,000 characters)'}), 400

        # If no scene_context provided, use cached data
        if not scene_context and cached_scene_data['scene_data']:
            scene_context = cached_scene_data['scene_data']

        if not question:
            return jsonify({'error': 'No question provided'}), 400

        print(f"\n{'='*60}")
        print(f"Question: {question}")
        print(f"{'='*60}")

        # Retrieve relevant documentation
        contexts = rag.retrieve_context(question, n_results=3)

        if contexts:
            print(f"[RAG] OK: Retrieved {len(contexts)} relevant docs")
            context_section = "\n\n".join([
                f"### {ctx['signature']}\n{ctx['text']}"
                for ctx in contexts
            ])
        else:
            print("[RAG] Warning: No RAG context available")
            context_section = "(No specific documentation found)"

        # Format scene context
        scene_summary = ""
        if scene_context:
            scene_summary = f"""
Current Scene Information:
- Objects: {scene_context.get('object_count', 0)} total
- Active: {scene_context.get('active_object', 'None')}
- Mode: {scene_context.get('mode', 'OBJECT')}
"""

        # Educational prompt
        system_prompt = f"""You are a patient Blender instructor helping students learn 3D modeling through the Blender interface.

CRITICAL INSTRUCTION: You MUST teach using UI-based instructions only. NEVER provide Python code or bpy commands.

Your teaching style:
- Provide step-by-step UI instructions (menu clicks, keyboard shortcuts, tool selections)
- Explain which menus to use (Add > Mesh > ..., Modifier Properties > Add Modifier > ...)
- Describe what buttons to click and what values to adjust in the properties panels
- Use clear descriptions like "In the 3D Viewport, press Shift+A, then select Mesh > UV Sphere"
- Explain concepts clearly and simply, using analogies when helpful
- Break down complex tasks into numbered steps
- Encourage experimentation with different settings
- Focus on understanding WHY each step matters, not just WHAT to do

{scene_summary}

The documentation below contains Python code for reference ONLY - you must translate these concepts into UI actions:
{context_section}

Answer the student's question in a friendly, educational manner with UI-based instructions. Keep answers concise (2-4 paragraphs).

EXAMPLES OF GOOD RESPONSES:
- "To add a sphere, press Shift+A in the 3D Viewport, then navigate to Mesh > UV Sphere"
- "In the Modifier Properties panel (wrench icon), click Add Modifier and select Bevel"
- "Select your object, press Tab to enter Edit Mode, then press Ctrl+R to add an edge loop"

NEVER write responses like this:
- "Use bpy.ops.mesh.primitive_uv_sphere_add(radius=1.0)"
- "Run this Python code: ..."
- Any Python code snippets or bpy commands"""

        user_prompt = f"""Question: {question}

Provide a clear, educational answer that helps the student understand this Blender concept."""

        # Call Ollama
        print("[Ollama] Info: Calling Ollama for educational response...")
        response = call_ollama(
            system_prompt,
            user_prompt,
            model=model,
            temperature=0.7
        )

        print("[Ollama] OK: Answer generated successfully")
        print(f"{'='*60}\n")

        return jsonify({
            'answer': response.strip(),
            'contexts_used': len(contexts),
            'rag_enabled': rag.initialized
        })

    except Exception as e:
        error_msg = str(e)
        print(f"[Ask] Error: Request failed - {error_msg}")
        traceback.print_exc()
        return jsonify({'error': error_msg}), 500


@app.route('/scene_analysis', methods=['POST'])
def analyze_scene():
    """Analyze scene and suggest next steps for learning."""
    try:
        # Validate input
        data = request.json
        if data is None:
            return jsonify({'error': 'Invalid JSON or Content-Type must be application/json'}), 400

        goal = data.get('goal', 'learning blender')
        scene_data = data.get('scene_data', {})
        model, model_error = validate_model_name(data.get('model'))
        if model_error:
            return jsonify({'error': model_error}), 400

        # Validate goal
        if not isinstance(goal, str):
            return jsonify({'error': 'Goal must be a string'}), 400

        goal = goal.strip()

        # Enforce input length limits
        if len(goal) > 500:
            return jsonify({'error': 'Goal too long (max 500 characters)'}), 400

        # Validate scene_data is a dict
        if not isinstance(scene_data, dict):
            return jsonify({'error': 'Scene data must be an object'}), 400

        print(f"\n{'='*60}")
        print(f"Scene Analysis - Goal: {goal}")
        print(f"Objects in scene: {scene_data.get('object_count', 0)}")
        print(f"{'='*60}")

        # Format scene info
        objects_list = "\n".join([
            f"  - {obj['name']} ({obj['type']})" +
            (f" with {len(obj.get('modifiers', []))} modifiers" if obj.get('modifiers') else "")
            for obj in scene_data.get('objects', [])
        ])

        scene_summary = f"""Current Scene:
- Total objects: {scene_data.get('object_count', 0)}
- Active object: {scene_data.get('active_object', 'None')}
- Mode: {scene_data.get('mode', 'OBJECT')}
- Render engine: {scene_data.get('render_engine', 'Unknown')}

Objects:
{objects_list if objects_list else '  (empty scene)'}
"""

        # Educational suggestion prompt
        system_prompt = f"""You are a Blender instructor analyzing a student's scene to suggest what they should learn next.

{scene_summary}

Your task:
- Analyze what the student has already done
- Suggest 3-5 concrete next steps they could take to learn more
- Focus on natural progression (basics → intermediate → advanced)
- Each suggestion should be a learning opportunity
- Keep suggestions action-oriented and specific

Provide suggestions as a numbered list. Each suggestion should be ONE sentence that starts with an action verb."""

        user_prompt = f"""The student's goal is: {goal}

Based on their current scene, what should they try next to continue learning? Provide 3-5 specific suggestions."""

        # Call Ollama
        print("[Ollama] Info: Generating scene analysis suggestions...")
        response = call_ollama(
            system_prompt,
            user_prompt,
            model=model,
            temperature=0.7
        )

        # Parse numbered list into array
        # Expected format: "1. First suggestion\n2. Second suggestion\n..."
        suggestions_list = []
        for line in response.strip().split('\n'):
            line = line.strip()
            if not line:
                continue
            # Remove leading number and punctuation (e.g., "1.", "1)", "1 -")
            cleaned = re.sub(r'^\d+[\.\)\-\:]\s*', '', line)
            if cleaned:
                suggestions_list.append(cleaned)

        print("[Ollama] OK: Suggestions generated successfully")
        print(f"{'='*60}\n")

        return jsonify({
            'suggestions': suggestions_list,
            'scene_summary': scene_summary
        })

    except Exception as e:
        error_msg = str(e)
        print(f"[SceneAnalysis] Error: Request failed - {error_msg}")
        traceback.print_exc()
        return jsonify({'error': error_msg}), 500


@app.route('/test', methods=['GET'])
def test():
    """Test endpoint."""
    return jsonify({
        'message': 'RAG Server is running!',
        'rag_enabled': rag.initialized,
        'endpoints': ['/health', '/rag/retrieve', '/scene/update', '/scene/current', '/ask', '/scene_analysis', '/test']
    })


def main():
    """Start the server."""
    print("\n" + "="*60)
    print("Blender Learning Assistant - RAG Server")
    print("="*60)
    print(f"Running at: http://127.0.0.1:5179")
    print("This server is OFFLINE - only accessible from this computer")
    print("")
    print("Educational Mode:")
    print("  - RAG retrieval: POST /rag/retrieve")
    print("  - Q&A endpoint: POST /ask")
    print("  - Scene analysis: POST /scene_analysis")
    print("  - Scene update: POST /scene/update")
    print("  - Scene current: GET /scene/current")
    print("")
    print(f"Model: {os.getenv('OLLAMA_MODEL', 'qwen2.5:7b-instruct-q4_K_M')}")
    print("="*60 + "\n")

    # Try to initialize RAG
    if rag.initialize():
        print("[Server] OK: RAG system ready (API documentation loaded)\n")
    else:
        print("[Server] Warning: RAG disabled - will use LLM knowledge only\n")

    print("Press Ctrl+C to stop the server\n")

    # Run server
    app.run(host='127.0.0.1', port=5179, debug=False)


if __name__ == '__main__':
    main()
