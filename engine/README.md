# Engine Zone

Shared inference platform for all SmolPC apps.

## Owned Responsibilities

- startup and readiness lifecycle
- backend/model selection and status
- inference execution and cancellation
- API contract and compatibility gates

## Components

- `engine/crates/smolpc-engine-core`
- `engine/crates/smolpc-engine-host`
- `engine/crates/smolpc-engine-client`

## Key References

- `docs/ENGINE_API.md`
- `docs/ENGINE_STANDALONE.md`
- `docs/APP_ONBOARDING_PLAYBOOK.md`
- `docs/ARCHITECTURE.md`
