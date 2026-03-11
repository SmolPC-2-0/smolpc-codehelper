# Blender Assistant Monorepo Move Checklist

Source app: `C:\Users\siddh\Desktop\smolpc-blenderhelper`  
Target app root: `C:\Users\siddh\Desktop\smolpc-codehelper\apps\blender-assistant`

## What I Confirmed

- `apps/blender-assistant/README.md` in the target monorepo is currently a skeleton only.
- The skeleton points to contract-first integration docs:
  - `docs/APP_ONBOARDING_PLAYBOOK.md`
  - `docs/ENGINE_API.md`
- The source app is a full Tauri + Svelte project and includes local heavy runtime artifacts that should not be copied directly into git:
  - `node_modules/`
  - `dist/`
  - `src-tauri/target/`
  - `src-tauri/resources/models/`
  - `src-tauri/libs/`
  - `src-tauri/binaries/`

## Prepared Copy Script

Script: `scripts/prepare-monorepo-move.ps1`

- Dry run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\prepare-monorepo-move.ps1
```

- Apply copy:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\prepare-monorepo-move.ps1 -Apply
```

The script copies the app into `apps/blender-assistant` and recreates `.gitkeep` placeholders for:

- `src-tauri/libs`
- `src-tauri/binaries`
- `src-tauri/resources/models`

## Required Monorepo Follow-Up (After Copy)

1. Add `apps/blender-assistant` to root npm workspaces in `smolpc-codehelper/package.json`.
2. Add `apps/blender-assistant/src-tauri` to root Cargo workspace members in `smolpc-codehelper/Cargo.toml`.
3. Add root scripts (optional but recommended) mirroring codehelper patterns:
   - `tauri:dev:blender`
   - `check:blender`
4. Decide whether Blender app should keep optional Ollama fallback or be strictly shared-engine contract only.
5. Run validation from target repo:
   - `npm install`
   - `npm run check --workspace apps/blender-assistant`
   - `cargo check -p blender_helper`

## Current Risk To Review Before Applying

- `C:\Users\siddh\Desktop\smolpc-codehelper` currently has an untracked top-level `src-tauri/` directory (`gen`, `target`), which is unrelated to `apps/blender-assistant` but should be kept in mind during cleanup/commit.
