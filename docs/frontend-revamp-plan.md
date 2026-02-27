# Frontend Revamp Tracking

This document tracks the implementation guardrails for the Workbench Bold frontend revamp.

## Must-Not-Break Checklist

- Chat persistence and chat switching behavior
- Background streaming while switching chats
- Cancel generation behavior and UI recovery state
- Model switching flow and disabled states during generation
- Context toggle persistence and runtime behavior
- Hardware indicator and hardware panel access
- Benchmark panel toggle shortcut (`Ctrl/Cmd + Shift + B`)
- Keyboard sending behavior (`Enter` to send, `Shift+Enter` for newline)
- Startup model listing and auto-load behavior

## Verification Gates

### Gate 0 (Pre-change baseline)

- `npm run check` passes

### Gate 1 (Design system + theme wiring)

- Tokens render correctly in light/dark
- Runtime theme switching works (`light`, `dark`, `system`)
- OS theme changes are respected when in `system`

### Gate 2 (Core shell refactor)

- New componentized shell works with unchanged store contracts
- Scroll, streaming, cancel, and sidebar interactions match baseline behavior

### Gate 3 (Core surface redesign)

- Updated visuals for: sidebar, messages, composer, quick examples, status/model/context controls
- Keyboard navigation and focus visibility pass manual checks
- Mobile and narrow-width layouts remain usable

### Gate 4 (Legacy cleanup)

- No runtime references remain to legacy static frontend files
- Legacy files removed: `src/index.html`, `src/main.js`, `src/styles.css`
- App runs/builds with active Svelte path only
