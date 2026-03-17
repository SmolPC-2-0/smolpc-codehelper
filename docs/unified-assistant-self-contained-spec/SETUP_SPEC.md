# Setup Subsystem Spec

**Last Updated:** 2026-03-17
**Status:** Phase 2 foundation contract, now merged; single-mainline workflow does not change the setup API

## 1. Purpose

The setup subsystem is the app-level foundation for self-contained delivery.

It does three things:

- reports whether the app-owned foundation pieces are ready
- prepares app-local setup state when bundled payloads are available
- gives later provider-specific provisioning phases one shared place to plug into

It is not a replacement for per-mode `mode_status`.

## 2. Phase 2 Scope

Phase 2 setup work is limited to:

- app-level setup state
- setup commands
- host-app detection
- packaged resource manifest validation
- bundled Python preparation scaffolding
- bundled model readiness validation
- one lightweight setup banner and setup panel

Phase 2 closeout status:

- backend setup subsystem landed
- `setup_status` and `setup_prepare` landed
- setup banner and setup panel landed
- current mode behavior remained unchanged

Phase 2 setup work does not include:

- Blender addon provisioning
- GIMP plugin/server provisioning
- host-app launch
- LibreOffice runtime switchover to bundled Python
- any mode activation change

## 3. Public Commands

Phase 2 adds:

- `setup_status`
- `setup_prepare`

### `setup_status`

Returns the current app-level setup snapshot.

### `setup_prepare`

Prepares the app-owned foundation items that can be prepared in Phase 2, then
returns the updated app-level setup snapshot.

`setup_prepare` in Phase 2 must:

- validate tracked resource manifests
- create app-local setup directories
- prepare bundled Python from packaged payloads when present
- validate the packaged model resource contract
- refresh host-app detection

`setup_prepare` in Phase 2 must not:

- launch GIMP, Blender, or LibreOffice
- mutate Blender or GIMP user profiles
- provision addons or plugins
- replace existing mode-specific commands

## 4. Public DTOs

```ts
type SetupItemState = 'ready' | 'missing' | 'not_prepared' | 'error';

interface SetupItemDto {
	id: string;
	label: string;
	state: SetupItemState;
	detail: string | null;
	required: boolean;
	canPrepare: boolean;
}

interface SetupStatusDto {
	overallState: 'ready' | 'needs_attention' | 'error';
	items: SetupItemDto[];
	lastError: string | null;
}
```

`detail` should be non-null whenever the app needs to explain why an item is
missing, not prepared, or in error. It may be `null` for clean ready states.

## 5. Locked Phase 2 Setup Item Ids

Phase 2 uses these item ids and no others:

- `engine_runtime`
- `bundled_model`
- `bundled_python`
- `host_gimp`
- `host_blender`
- `host_libreoffice`

Expected meanings:

- `engine_runtime`: current engine runtime path contract is usable
- `bundled_model`: packaged default model manifest and resource path resolve
- `bundled_python`: packaged Python manifest exists and the app-local runtime is ready or prepare-able
- `host_gimp`: GIMP install detected or missing
- `host_blender`: Blender install detected or missing
- `host_libreoffice`: LibreOffice / Collabora install detected or missing

## 6. Setup State Rules

The setup subsystem is app-level, not mode-level.

That means:

- setup state can show that the app needs attention without breaking shell load
- host-app absence is reported honestly, not as a fatal app startup error
- modes continue to use their own provider status and error paths
- Phase 2 keeps current mode composer availability unchanged

## 7. Detection Rules

Windows host-app detection order:

1. cached resolved path from setup state, if still valid
2. Windows App Paths or registry lookup when available
3. standard install directories
4. `PATH` lookup

Phase 2 detection is read-only:

- it does not launch host apps
- it does not repair host app installs
- it does not write provider-specific user-profile files

## 8. Resource And App-Local Layout

Phase 2 resource roots:

- `resources/python/`
- `resources/gimp/`
- `resources/blender/`
- `resources/libreoffice/`
- `resources/models/`

Each root must have a tracked manifest with:

- `version`
- `source`
- `expectedPaths`
- `status`

Phase 2 app-local setup roots:

- `setup/python/`
- `setup/state/`
- `setup/logs/`

## 9. Frontend Contract

Phase 2 adds:

- one background setup-store initialization
- one setup banner when `overallState !== 'ready'`
- one setup panel with item-level detail and a single `Prepare` action

The setup surface must remain lightweight:

- no settings page migration
- no blocking first-run wizard
- no mode routing changes
- no chat partitioning changes

## 10. Later-Phase Handoff

Later phases build on this subsystem:

- Phase 3: LibreOffice uses bundled Python ownership from setup
- Phase 4: Blender addon provisioning plugs into setup/provision state
- Phase 5: GIMP plugin/server provisioning plugs into setup/provision state
- Phase 6: packaged release validation uses setup status as the first-run and repair surface

The single-mainline workflow adopted after Phase 2 does not change the
`setup_status` or `setup_prepare` wire contract. It only changes where the
future docs PRs land.

The next official branch after Phase 2 closeout is:

- `codex/unified-self-contained-libreoffice-docs`
