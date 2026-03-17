# Self-Contained Unified Assistant Delivery Plan

**Last Updated:** 2026-03-17
**Status:** Approved master roadmap for the self-contained line

## Goal

Deliver a Windows build of `SmolPC Unified Assistant` that external users can
install and use without manual setup beyond installing the host applications
themselves.

Allowed external installs:

- GIMP
- Blender
- LibreOffice / Collabora

Not allowed as manual user steps:

- Python installation
- model download/setup
- MCP server setup
- Blender addon install
- GIMP plugin/server install
- terminal commands
- environment variables

## Finish Line

The app is considered self-contained when all of these are true:

- Code mode works immediately after install
- GIMP mode provisions what it needs, launches GIMP if needed, and executes a real edit
- Blender mode provisions its addon, launches Blender if needed, and completes a scene-aware workflow
- Writer mode launches the bundled runtime plus LibreOffice and edits a document
- Slides mode launches the bundled runtime plus LibreOffice and edits a presentation
- Calc remains visible but intentionally disabled

## Product Rules

- one installer
- one app binary
- one shared engine runtime
- one bundled default model: `qwen3-4b-instruct-2507`
- one app-private Python runtime
- bundle identifier remains `com.smolpc.codehelper`
- Windows only

## Workstream Rules

- frozen demo baseline stays available for demos
- self-contained work happens only on:
  - `dev/unified-assistant-self-contained`
  - `docs/unified-assistant-self-contained-spec`
- no self-contained work is merged back into the frozen demo line
- third-party provenance must be documented before asset import

## Runtime Ownership By Mode

| Mode    | Host app                | Bundled by unified app         | Auto-provisioned by unified app | Auto-launched by unified app |
| ------- | ----------------------- | ------------------------------ | ------------------------------- | ---------------------------- |
| Code    | none                    | engine + default model         | n/a                             | yes                          |
| GIMP    | GIMP                    | plugin/server runtime snapshot | yes                             | yes                          |
| Blender | Blender                 | addon payload + bridge assets  | yes                             | yes                          |
| Writer  | LibreOffice / Collabora | Python runtime + MCP scripts   | yes                             | yes                          |
| Slides  | LibreOffice / Collabora | Python runtime + MCP scripts   | yes                             | yes                          |
| Calc    | LibreOffice / Collabora | scaffold only                  | no                              | no                           |

## Ordered Plan

1. Branch cut and freeze the demo baseline.
2. Document the self-contained target completely.
3. Add setup/provisioning foundation.
4. Remove external Python through bundled runtime ownership.
5. Bundle and provision the Blender addon.
6. Vendor and provision the GIMP plugin/server runtime.
7. Finalize packaging and validate on clean Windows machines.

## Acceptance Checklist

- no manual dependency setup beyond host-app install
- no launcher dependency
- no system Python requirement
- bundled default model available offline
- versioned provisioning/repair behavior in place
- upgrade path preserves chat data and only reprovisions when asset versions change

## Explicitly Deferred

- Calc activation
- bundle identifier migration
- multi-model packaging strategy beyond one default bundled model
- macOS/Linux shipping
- post-v1 feature expansion
