# Engine Bundling Setup

Use this when producing a self-contained build that includes:

- Engine sidecar binary
- ONNX runtime libraries
- Model artifacts staged under `src-tauri/resources/models`

## 1) Runtime Libraries (`src-tauri/libs`)

If libs are missing, fetch them:

```bash
./scripts/setup-libs.sh
```

## 2) Engine Host Binary (`src-tauri/binaries`)

Build/copy `smolpc-engine-host(.exe)` into `src-tauri/binaries/`.

The app resolves this path at startup and spawns/reuses the engine on `127.0.0.1:19432`.

## 3) Prepare Model Artifacts

Choose one model setup path:

```powershell
npm run model:setup:qwen2_5
# or
npm run model:setup:qwen3
```

By default, scripts populate `%LOCALAPPDATA%\SmolPC\models\<model-id>\...`.

## 4) Stage Model Artifacts into Bundle Resources

```powershell
npm run bundle:stage:model
```

This copies/hard-links from local model storage into:

```text
src-tauri/resources/models/<model-id>/...
```

You can override model id/source using script flags in `scripts/stage-bundled-model.ps1`.

## 5) Build Self-Contained Release

```powershell
npm run tauri:build:self-contained
```

`tauri:build:self-contained` runs staging first, then `tauri build`.

## Runtime Resolution Notes

Shared engine model directory resolution order:

1. `SMOLPC_MODELS_DIR`
2. Bundled `models` resources
3. `%LOCALAPPDATA%\SmolPC\models`
4. Dev fallback paths

## Verification

1. Launch built app.
2. Confirm logs show engine spawn/reuse and model load attempt.
3. Check bridge health:

```powershell
Invoke-RestMethod http://127.0.0.1:5179/health
```

Expected fields include `backend`, `model`, `connected`, `generating`, `rag_enabled`, `rag_docs`.

## Common Failures

- Missing engine binary in `src-tauri/binaries`
- Missing runtime libs in `src-tauri/libs`
- Missing staged model files in `src-tauri/resources/models`
- Large model artifact exceeds installer packaging constraints (use unpacked release output when needed)
