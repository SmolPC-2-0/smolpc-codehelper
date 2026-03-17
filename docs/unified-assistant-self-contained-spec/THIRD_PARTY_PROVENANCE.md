# Third-Party Provenance Tracker

**Last Updated:** 2026-03-17
**Status:** Required tracker before third-party runtime imports land on the self-contained line

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

| Component                                      | Current source                                                                                     | Pin status     | License status                                                    | Import status                             | Notes                                                                                       |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------- | -------------- | ----------------------------------------------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------------------- |
| GIMP MCP/plugin runtime                        | upstream `gimp-mcp` source snapshot                                                                | pending        | pending                                                           | not yet imported into self-contained line | Required before Phase 5 implementation                                                      |
| Blender addon payload                          | `apps/blender-assistant/blender_addon/blender_helper_http.py`                                      | in-repo source | repo license review pending                                       | not yet repackaged into unified resources | Phase 4 will provision from existing repo source                                            |
| LibreOffice MCP runtime scripts                | imported from `origin/codex/libreoffice-port-track-a` @ `7acad1fa0eb31e32a5485069e85c021d14284455` | pinned         | same repo lineage; formal release packaging review still required | already present in unified resources      | Imported from the same repository line; Phase 3 switches them onto bundled Python ownership |
| Bundled Python runtime                         | packaging-side managed runtime plus wheels                                                         | pending        | pending                                                           | not yet added                             | Phase 2 foundation import                                                                   |
| Bundled `uv` tooling/runtime support           | packaging-side toolchain                                                                           | pending        | pending                                                           | not yet added                             | Phase 2 foundation import                                                                   |
| Default bundled model `qwen3-4b-instruct-2507` | current engine-supported model artifact source                                                     | pending        | pending                                                           | not yet bundled                           | Phase 2 foundation import; exact packaged artifact validation is still required             |

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
