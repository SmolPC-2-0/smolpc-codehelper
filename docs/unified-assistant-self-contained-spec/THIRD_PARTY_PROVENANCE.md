# Third-Party Provenance Tracker

**Last Updated:** 2026-03-17
**Status:** Required tracker with Phase 3 bundled-Python source contract locked for LibreOffice

## Purpose

Every bundled third-party runtime, plugin, addon, helper, wheel set, or model
artifact must be tracked here before it is imported into the self-contained
line.

Required fields:

- source repository or upstream distribution
- exact pinned commit, tag, or release
- license
- files or directories imported
- local modifications expected
- validation owner/status

## Tracking Table

| Component                                      | Current source                                                                                                  | Pin status     | License status                                                    | Import status                             | Notes                                                                                                                                                                 |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | -------------- | ----------------------------------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GIMP MCP/plugin runtime                        | upstream `gimp-mcp` source snapshot                                                                             | pending        | pending                                                           | not yet imported into self-contained line | Required before Phase 5 implementation                                                                                                                                |
| Blender addon payload                          | `apps/blender-assistant/blender_addon/blender_helper_http.py`                                                   | in-repo source | repo license review pending                                       | not yet repackaged into unified resources | Phase 4 will provision from existing repo source                                                                                                                      |
| LibreOffice MCP runtime scripts                | imported from `origin/codex/libreoffice-port-track-a` @ `7acad1fa0eb31e32a5485069e85c021d14284455`              | pinned         | same repo lineage; formal release packaging review still required | already present in unified resources      | Imported from the same repository line; Phase 3 switches them onto bundled Python ownership                                                                           |
| Bundled Python runtime                         | official Windows x64 CPython embeddable distribution from `python.org`, staged into `resources/python/payload/` | source locked  | Python Software Foundation License                                | packaged-mode contract is live            | Phase 3 runtime code now consumes the prepared bundled runtime for Writer/Slides; exact staged CPython release still needs a manifest pin when payloads are populated |
| Bundled `uv` tooling/runtime support           | Astral `uv` Windows binary staged alongside the bundled Python payload                                          | source locked  | Apache-2.0 OR MIT                                                 | manifest/staging contract landed          | Used for packaged Python management and future offline wheel install/repair flows; exact staged binary release still needs a manifest pin when payloads are populated |
| Default bundled model `qwen3-4b-instruct-2507` | current engine-supported model artifact source                                                                  | pending        | pending                                                           | manifest/staging contract landed          | Phase 2 added manifests and staging hooks; exact packaged artifact validation is still required                                                                       |

## Phase 2-3 Provenance Rule

Phases 2 and 3 may add manifests, staging hooks, and runtime ownership code for
bundled Python and the default model, but they should not silently treat those
artifacts as fully cleared for release packaging. The tracker must stay honest
about:

- exact pinned packaged artifact source
- license review status
- whether the payload is committed, staged at build time, or still pending
- whether Windows packaged validation has actually been performed

## Per-Component Template

### Component Name

- **Source:** `TBD`
- **Pinned version/commit:** `TBD`
- **License:** `TBD`
- **Imported files:** `TBD`
- **Local modifications:** `TBD`
- **Validation owner:** `TBD`
- **Status:** `TBD`

## Rule

No third-party runtime or plugin bundle should land in
`dev/unified-assistant-self-contained` until its row is filled out here with a
real pin and license note.
