# Bundled Python Payload Contract

This directory defines the packaged resource contract for the app-private
Python runtime used by the self-contained line.

The dev and packaging contract is pinned to:

- the official Windows x64 CPython 3.12.9 embeddable distribution from `python.org`
- the pinned `uv` 0.10.12 Windows binary from Astral
- staged runtime files under `payload/`

The staged payload stays out of git history. Developers should populate it with:

```powershell
npm run runtime:setup:python
```

That command stages the pinned embeddable Python runtime plus `uv.exe` and
`uvx.exe` into this directory so `Prepare` can copy the payload into app-local
setup state during `npm run tauri:dev`.

Expected staged contents:

- `payload/python.exe`
- `payload/uv.exe`
- `payload/uvx.exe`
- the rest of the embeddable CPython runtime files extracted alongside them
