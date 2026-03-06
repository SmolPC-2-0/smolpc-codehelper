# ORT Docs: Relevant Notes For SmolPC

Date: 2026-03-06  
Focus: `ort.pyke.io` + `docs.rs/ort` behavior that impacts engine-host EP selection.

## 1) EP Registration and Fallback

- EPs are registered in the order provided.
- ORT can fall back across providers when operators are unsupported by earlier EPs.
- `ort` docs explicitly note silent CPU fallback when EP registration fails unless configured otherwise.
- Use `.error_on_failure()` on non-CPU EP candidates to avoid hidden downgrade in host selection flow.

Sources:
- https://ort.pyke.io/perf/execution-providers
- https://docs.rs/ort/2.0.0-rc.12/ort/session/builder/struct.SessionBuilder.html

## 2) Session vs Environment EP Configuration

- EPs can be configured globally via `ort::init().with_execution_providers(...)`.
- Session-level EPs take precedence over environment-level EPs.
- `with_no_environment_execution_providers` exists and can be used to reduce implicit global behavior in strict host-controlled flows.

Sources:
- https://ort.pyke.io/perf/execution-providers
- https://docs.rs/ort/2.0.0-rc.12/src/ort/session/builder/impl_options.rs.html

## 3) Auto Device Selection APIs

- `with_auto_device(policy)` and `with_devices(...)` are available (`api-22+`).
- `AutoDevicePolicy` includes `PreferCPU`, `PreferNPU`, `PreferGPU`, `MaxPerformance`, `MaxEfficiency`, `MinPower`.
- Current docs indicate:
  - `Default` ~= `PreferCPU`
  - `MaxPerformance` ~= `PreferGPU`
  - `MaxEfficiency` and `MinPower` ~= `PreferNPU`

Implication:
- Useful for discovery/prototyping, but production policy should remain explicit and host-owned to preserve SmolPC contract boundaries.

Sources:
- https://docs.rs/ort/2.0.0-rc.12/ort/session/builder/struct.SessionBuilder.html
- https://docs.rs/ort/2.0.0-rc.12/ort/session/builder/enum.AutoDevicePolicy.html

## 4) Linking and Runtime Control

- `load-dynamic` is recommended by `ort` docs for flexibility and lower operational friction.
- Runtime path can be set via `ORT_DYLIB_PATH`.
- Compile-time dynamic linking requires platform-specific dylib search-path handling.

Implication:
- Prefer host-controlled absolute runtime paths + validated package roots for security and deterministic deployment.

Source:
- https://ort.pyke.io/setup/linking

## 5) Feature/Build Constraints

- EP usage requires Cargo feature enablement and matching ORT build support.
- `ort` prebuilt artifact combinations are constrained.
- Current `dist.txt` references `ms@1.24.2`; OpenVINO-specific prebuilt lane is not present in that channel snapshot.

Implication:
- OpenVINO production rollout should use custom package/build pipeline and tuple gating.

Sources:
- https://ort.pyke.io/perf/execution-providers
- https://github.com/pykeio/ort/blob/main/ort-sys/build/download/dist.txt

## 6) Practical Guardrails For SmolPC Engine Host

1. Always define explicit EP candidate order in host.
2. Use `.error_on_failure()` for acceleration candidates.
3. Keep CPU as explicit terminal fallback candidate.
4. Emit reasoned selection and demotion telemetry.
5. Pin and validate runtime tuples (ORT + EP + vendor runtime), do not auto-upgrade silently.

