---
paths:
  - "scripts/**/*"
  - "*.ps1"
  - "*.sh"
---

# Build & Toolchain Rules

- PowerShell wrappers around native tools must coerce stderr records to plain strings before logging or `$ErrorActionPreference = 'Stop'` will treat normal tool output as a fatal error
- After a long-running model export times out at the shell layer, check for orphaned builder `python` processes before retrying or the next validation pass starts from a dirty state
