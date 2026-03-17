# Model Strategy For The Self-Contained Line

**Last Updated:** 2026-03-17
**Status:** Bundled default model decision is locked for self-contained delivery

## 1. Shipping Decision

The self-contained line ships one bundled default model:

- `qwen3-4b-instruct-2507`

Why this is locked:

- it is already first in the current engine registry ordering
- it is already the built-in default chosen by engine startup when no override is provided
- it gives a single offline baseline that works across the live unified modes
- it avoids opening a second large productization track around multi-model packaging

Locking this product decision does not mean every future packaged artifact
variant is already validation-complete. Phase 2 must still verify the exact
packaged `qwen3-4b-instruct-2507` artifact used on the self-contained line,
including Windows packaging behavior and target hardware readiness.

Phase 2 must also define the build-artifact staging contract for this bundled
model. The model remains the locked packaged default, but staging, packaging
validation, and Windows runtime validation are still required before the
self-contained line can treat it as release-ready.

## 2. Finish-Line Rule

External users must not manually install or fetch models.

That means:

- the installer bundles one default model
- the engine auto-start path resolves that model automatically
- setup status can report if the packaged model is missing or damaged

Phase 2 foundation requirement:

- the packaged app must have a defined resource contract at `resources/models/`
- the self-contained line keeps the current engine behavior that prefers a bundled
  `resource_dir/models` when present
- Phase 2 validates that contract; it does not change model ordering or inference DTOs

## 3. Fallback And Future Policy

Allowed in the future, but not part of this finish line:

- optional additional bundled models
- optional downloadable model packs
- hardware-tier-specific multi-model bundles
- automatic post-install model upgrade flows

## 4. Relationship To Current Registry

Current engine-supported models include:

- `qwen3-4b-instruct-2507`
- `qwen2.5-coder-1.5b`
- `qwen3-4b-int4-ov`
- `qwen3-4b-int4-ov-npu`

The self-contained line does not promise all of these are bundled. It promises
one guaranteed offline baseline:

- `qwen3-4b-instruct-2507`

## 5. Why Not Multi-Model Bundling Now

Bundling multiple models would add cost in all of these areas:

- installer size
- resource staging complexity
- first-run validation
- documentation
- support matrix

The self-contained finish line is about removing user setup, not solving every
future hardware-tier optimization in the same branch stack.

## 6. Validation Requirements

The self-contained line must validate:

- the packaged default model path resolves correctly
- engine startup can load the bundled default model
- no external model setup is required
- missing/damaged model resources produce honest setup status

Phase 2 additionally requires:

- build-artifact staging for the bundled model is defined and scripted
- packaged artifact selection for `qwen3-4b-instruct-2507` is pinned and reviewable
- Windows runtime validation is still treated as required follow-up, not assumed complete

## 7. Deferred Questions

Post-finish-line only:

- whether a smaller bundled fallback model should be added for low-RAM devices
- whether OpenVINO-specific model bundles should ship alongside the default
- whether separate IT-admin offline packs should exist
