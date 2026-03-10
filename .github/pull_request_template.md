## Summary

- What changed:
- Why:

## Zone Impact

- [ ] `engine/`
- [ ] `launcher/`
- [ ] `apps/`
- [ ] `docs/`

## Boundary Compliance

- [ ] No app-owned inference implementation added.
- [ ] No direct dependency on engine host internals from apps/launcher.
- [ ] `npm run boundary:check` passes locally.

## Validation

- [ ] `npm run check`
- [ ] `cargo test -p smolpc-engine-core -p smolpc-engine-client -p smolpc-engine-host`
- [ ] `cargo check -p smolpc-code-helper`

## Documentation

- [ ] Updated relevant zone README(s).
- [ ] Updated contract docs (`docs/ENGINE_API.md`) if interface changed.
- [ ] Updated onboarding docs if integration flow changed.
