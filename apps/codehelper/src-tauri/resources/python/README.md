# Bundled Python Payload Contract

This directory defines the packaged resource contract for the app-private
Python runtime used by the self-contained line.

Phase 3 locks the delivery source to:

- the official Windows x64 CPython embeddable distribution from `python.org`
- a pinned `uv` Windows binary from Astral
- provider-owned wheel/runtime inputs staged into `payload/`

The final large runtime payload still stays out of git history. Packaging-stage
scripts populate `payload/` at build time.

Expected future staged contents:

- `payload/`
- runtime wheels or environment inputs needed for bundled execution
