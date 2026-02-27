# DirectML Export and Model Layout

## Required layout

```
src-tauri/models/<model_id>/
├── cpu/
│   └── model.onnx
├── dml/
│   └── model.onnx
└── tokenizer.json
```

CPU has a legacy fallback to `src-tauri/models/<model_id>/model.onnx`.

DirectML does not use legacy fallback. `dml/model.onnx` must exist.

## Canonical DML export path

Use ONNX Runtime GenAI builder targeting DirectML:

```powershell
npm run model:export:dml
```

or:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\export-dml-model.ps1
```

## Notes

- Default script target:
  - HF model: `Qwen/Qwen2.5-Coder-1.5B-Instruct`
  - output: `src-tauri/models/qwen2.5-coder-1.5b/dml`
  - precision: `int4`
- Ensure `onnxruntime-genai` Python package is installed before export.
