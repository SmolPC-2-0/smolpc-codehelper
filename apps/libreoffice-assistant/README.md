# LibreOffice Assistant

Planned app zone for LibreOffice assistant integration into the shared engine + launcher architecture.

Primary migration guide (includes pre-merge vs post-merge launcher tracks):

1. `docs/LIBREOFFICE_UNIFIED_LAUNCHER_PORTING_GUIDE.md`

Integration baselines:

1. `docs/APP_ONBOARDING_PLAYBOOK.md`
2. `docs/ENGINE_API.md`
3. `docs/SMOLPC_SUITE_INTEGRATION.md`

Placement rule:

1. All LibreOffice app code belongs under `apps/libreoffice-assistant/...`.
2. Do not place LibreOffice app source under root `/src-tauri` (root path is generated artifact residue, not an app zone).
