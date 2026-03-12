"""
Build Knowledge Base for Blender Helper AI

Downloads Blender API documentation and creates vector embeddings.
Run this ONCE before using the RAG server.
"""

import os
import json
import requests
from bs4 import BeautifulSoup
from pathlib import Path
import pickle
import numpy as np
try:
    from sentence_transformers import SentenceTransformer
except ImportError:
    print("[ERROR] sentence-transformers not installed")
    print("Install with: pip install -r requirements_server.txt")
    exit(1)
import time

# Configuration
BLENDER_VERSION = os.getenv("BLENDER_VERSION", "4.2")
DOCS_BASE_URL = f"https://docs.blender.org/api/{BLENDER_VERSION}"
DB_PATH = Path(__file__).parent / "simple_db"
CACHE_PATH = Path(__file__).parent / "docs_cache"

# Key API pages to index
API_PAGES = [
    "/bpy.ops.mesh.html",
    "/bpy.ops.object.html",
    "/bpy.types.Modifier.html",
    "/bpy.types.BevelModifier.html",
    "/bpy.types.SubsurfModifier.html",
    "/bpy.types.ArrayModifier.html",
    "/bpy.types.MirrorModifier.html",
    "/bpy.types.SolidifyModifier.html",
    "/bpy.types.BooleanModifier.html",
    "/bpy.types.Object.html",
    "/bpy.types.Mesh.html",
    "/bpy.types.Material.html",
    "/bpy.context.html",
    "/bpy.data.html",
]


class BlenderDocsIndexer:
    def __init__(self):
        self.cache_path = CACHE_PATH
        self.cache_path.mkdir(exist_ok=True)
        self.db_path = DB_PATH
        self.db_path.mkdir(exist_ok=True)

        print("Loading embedding model...")
        self.embedding_model = SentenceTransformer('all-MiniLM-L6-v2')

    def fetch_page(self, url_path):
        """Fetch and cache documentation page."""
        cache_file = self.cache_path / url_path.replace("/", "_")

        if cache_file.exists():
            print(f"  [CACHE] {url_path}")
            return cache_file.read_text(encoding='utf-8')

        full_url = DOCS_BASE_URL + url_path
        print(f"  [FETCH] {full_url}")

        try:
            response = requests.get(full_url, timeout=10)
            response.raise_for_status()
            html = response.text
            cache_file.write_text(html, encoding='utf-8')
            time.sleep(1)  # Be nice to server
            return html
        except Exception as e:
            print(f"  [ERROR] {e}")
            return None

    def parse_page(self, html, url_path):
        """Extract documentation chunks."""
        soup = BeautifulSoup(html, 'html.parser')
        chunks = []

        # Extract API references
        for section in soup.find_all(['dl', 'div'], class_=['function', 'method', 'attribute', 'class', 'data']):
            sig = section.find(['dt', 'h3', 'h4'])
            if not sig:
                continue

            signature = sig.get_text(strip=True)
            desc_elem = section.find(['dd', 'p'])
            description = desc_elem.get_text(strip=True) if desc_elem else ""

            chunk_text = f"{signature}\n\n{description}"
            chunks.append({
                "text": chunk_text,
                "signature": signature,
                "url": url_path,
            })

        # Extract code examples
        for code_block in soup.find_all('pre'):
            code = code_block.get_text(strip=True)
            if 'bpy.' in code and len(code) > 50:
                context_elem = code_block.find_previous(['p', 'h2', 'h3'])
                context = context_elem.get_text(strip=True) if context_elem else ""

                chunks.append({
                    "text": f"Example:\n{context}\n\n```python\n{code}\n```",
                    "signature": f"Example from {url_path}",
                    "url": url_path,
                })

        return chunks

    def build_database(self):
        """Build the knowledge base."""
        print(f"\n{'='*60}")
        print("Building Blender API Knowledge Base")
        print(f"{'='*60}\n")

        all_chunks = []

        for page_url in API_PAGES:
            print(f"\n[PROCESS] {page_url}")
            html = self.fetch_page(page_url)
            if not html:
                continue

            chunks = self.parse_page(html, page_url)
            print(f"  [OK] Extracted {len(chunks)} chunks")
            all_chunks.extend(chunks)

        if not all_chunks:
            print("\n[ERROR] No content extracted. Check internet connection.")
            return

        print(f"\n{'='*60}")
        print(f"[INFO] Total chunks: {len(all_chunks)}")
        print("[INFO] Creating embeddings...")
        print(f"{'='*60}\n")

        # Create embeddings
        texts = [chunk["text"] for chunk in all_chunks]
        print(f"Embedding {len(texts)} documents...")
        embeddings = self.embedding_model.encode(texts, show_progress_bar=True)

        # Save as NumPy array
        embeddings_file = self.db_path / "embeddings.npy"
        np.save(embeddings_file, embeddings)
        print(f"\n[OK] Embeddings saved: {embeddings_file}")

        # Save metadata
        metadata_file = self.db_path / "metadata.pkl"
        with open(metadata_file, 'wb') as f:
            pickle.dump(all_chunks, f)
        print(f"[OK] Metadata saved: {metadata_file}")

        # Save JSON metadata for Rust Tier 2 loader
        metadata_json_file = self.db_path / "metadata.json"
        with open(metadata_json_file, 'w', encoding='utf-8') as f:
            json.dump(all_chunks, f, ensure_ascii=False)
        print(f"[OK] Metadata JSON saved: {metadata_json_file}")

        print(f"\n{'='*60}")
        print(f"[OK] Built knowledge base with {len(all_chunks)} documents")
        print(f"{'='*60}")
        print(f"\n[INFO] Database location: {self.db_path}")
        print("\n[INFO] Next steps:")
        print("  1. Start server: python server.py")
        print("  2. Open Blender and use the addon")
        print()


def main():
    indexer = BlenderDocsIndexer()
    indexer.build_database()


if __name__ == "__main__":
    main()
