# SmolPC Unified Assistant -- Frontend Specification

**Last Updated:** 2026-03-16
**Status:** Canonical frontend spec for the unified app

## 1. Frontend Direction

The current `apps/codehelper` app shell is the canonical starting point for the
unified frontend. It is not being replaced by a launcher UI or split into a
separate code-only product. Instead, the existing shell becomes the shared UI
for six modes:

- Code
- GIMP
- Blender
- Writer
- Calc
- Slides

`Slides` is the UI label for the internal id `impress`.

## 2. Core Constraints

1. Svelte 5 runes only.
2. Tailwind 4 only.
3. One shared shell, not per-mode mini apps.
4. One shared engine connection.
5. Per-mode history, prompts, suggestions, and provider status.
6. Windows is the validation target.

## 3. Frontend Types

```ts
export type AppMode =
  | 'code'
  | 'gimp'
  | 'blender'
  | 'writer'
  | 'calc'
  | 'impress';

export interface ModeCapabilities {
  supportsTools: boolean;
  supportsUndo: boolean;
  showModelInfo: boolean;
  showHardwarePanel: boolean;
  showBenchmarkPanel: boolean;
  showExport: boolean;
  showContextControls: boolean;
}

export interface ModeConfig {
  id: AppMode;
  label: string;
  subtitle: string;
  icon: string;
  providerKind: 'local' | 'mcp' | 'hybrid';
  systemPromptKey: string;
  suggestions: string[];
  capabilities: ModeCapabilities;
}

export interface Chat {
  id: string;
  mode: AppMode;
  title: string;
  messages: Message[];
  createdAt: number;
  updatedAt: number;
  model: string;
  pinned?: boolean;
  archived?: boolean;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  isStreaming?: boolean;
  explain?: string | null;
  undoable?: boolean;
  toolResults?: ToolExecutionResultDto[];
  plan?: unknown;
  status?: 'pending' | 'complete' | 'error';
}

export interface ModeStatus {
  mode: AppMode;
  engineReady: boolean;
  providerState: ProviderStateDto;
  availableTools: ToolDefinitionDto[];
  lastError: string | null;
}

export type AssistantStreamEvent =
  | { kind: 'status'; phase: string; detail: string }
  | { kind: 'tool_call'; name: string; arguments: unknown }
  | { kind: 'tool_result'; name: string; result: ToolExecutionResultDto }
  | { kind: 'token'; token: string }
  | { kind: 'complete'; response: AssistantResponseDto }
  | { kind: 'error'; code: string; message: string };
```

## 4. Mode Configuration

| Mode | Label | Provider | Shared shell notes |
|---|---|---|---|
| `code` | Code | local | Preserves current Codehelper experience |
| `gimp` | GIMP | mcp | Adds tool status and undo affordances |
| `blender` | Blender | hybrid | Uses bridge-backed scene workflows |
| `writer` | Writer | mcp | LibreOffice submode |
| `calc` | Calc | mcp | LibreOffice submode |
| `impress` | Slides | mcp | LibreOffice submode with Slides label |

## 5. Shared Shell

The app has one persistent shell:

- app identity / mode title
- app-mode dropdown in the header
- history sidebar
- conversation view
- composer
- status bar
- optional diagnostics panels

The shell does not fork per mode. Mode switching only changes content and
capabilities.

### Shared layout

```text
App
  Header
    AppIdentity
    AppModeDropdown
    StatusBar
  Body
    Sidebar
      PerModeHistoryList
    ConversationArea
      SuggestionChips
      ConversationView
      ComposerBar
  Overlays
    ModelInfoPanel
    HardwarePanel
    BenchmarkPanel
```

## 6. Mode Switching Behavior

### Changes on mode switch

- active system prompt
- active provider status
- visible history list
- suggestion chips
- available tools
- undo affordance
- app label and subtitle

### Does not change on mode switch

- engine process
- loaded model unless explicitly changed by the user
- global app shell
- stored chats for other modes

### Phase 2 shell rule

Phase 2 does not activate real non-Code execution. Mode switching only changes
the shell state, provider status visibility, suggestions, and capability-driven
UI.

## 7. Per-Mode Histories

Chat history is stored in one shared store but filtered by `Chat.mode`.

Rules:

1. Every chat is tagged with exactly one mode.
2. The sidebar shows only chats matching the active mode.
3. Switching modes does not delete or migrate chats.
4. Unified storage must use fresh versioned keys and must not reuse
   `smolpc_chats` or `smolpc_current_chat`.
5. The unified app does not migrate existing standalone Codehelper chats.
6. Current chat state is tracked per mode, not as one global current chat id.

### Phase 2 unified storage keys

- chats: `smolpc_unified_chats_v1`
- current chat by mode: `smolpc_unified_current_chat_by_mode_v1`
- active mode: `smolpc_unified_active_mode_v1`

### Auto-chat creation rule

- On first empty boot, the shell auto-creates one Code chat only.
- Non-Code modes do not auto-create blank chats.
- In non-Code modes, chats are created only when the user explicitly clicks
  `New Chat`.

## 8. Capability Flags

Capability flags control which existing Codehelper surfaces stay visible in each
mode.

### Shared across all modes

- sidebar / chat history
- conversation view
- composer shell
- status bar
- model info panel
- hardware panel

### Code-only in v1

- benchmark panel
- export action
- code-oriented workspace controls
- code-specific quick actions

### Mode-specific

- undo button only where the mode supports it
- tool summary surfaces only where the provider supports tools
- disabled composer copy in modes that are visible but not yet wired

### Phase 2 capability handling

- benchmark and export stay Code-only
- model info and hardware panels stay shared
- context controls hide outside Code
- non-Code modes show suggestions and provider status, but do not submit prompts

## 9. Code Mode Preservation

Code mode is not a stripped-down shell. It preserves the current Codehelper
behavior that already exists in `apps/codehelper`.

What remains in Code mode:

- current chat shell and conversation behavior
- coding-focused assistant behavior and prompts
- export flow
- model and hardware diagnostics
- keyboard shortcuts
- code-specific panels and controls listed in [CODE_MODE_SPEC.md](CODE_MODE_SPEC.md)

### Phase 2 execution rule

During Phase 2 shell work, the existing Codehelper send/generate flow remains
active only when `activeMode === 'code'`.

### Phase 3 Code-mode rule

During Phase 3 Code-mode work:

- Code mode continues to use the existing Codehelper inference path.
- `assistant_send` remains scaffold-only and is not activated for Code mode.
- the visible header/shell status for active Code mode should come from the
  current inference/backend state rather than the scaffold provider copy.
- switching away from Code during generation is allowed and does not cancel or
  relocate the generation.

## 10. Suggestion Chips

Suggestion chips are mode-specific empty-state actions.

Examples:

- Code: "Explain this error", "Write a function", "Review this snippet"
- GIMP: "Resize an image", "Remove the background", "Undo the last change"
- Blender: "Explain this scene", "Create a simple material", "Fix this modifier"
- Writer: "Draft a paragraph", "Rewrite this passage", "Summarize this text"
- Calc: "Explain this formula", "Build a grade table", "Clean this data"
- Slides: "Draft slide bullets", "Turn notes into slides", "Improve this outline"

## 11. Tauri Command Contracts

These are documentation contracts for implementation. They must exist before
mode work begins.

### `list_modes()`

- Purpose: bootstrap mode configs into the frontend
- Arguments: none
- Returns: `ModeConfig[]`
- Failure mode: startup/config error only

### `mode_status(mode)`

- Purpose: fetch engine and provider status for one mode
- Arguments: `{ mode: AppMode }`
- Returns: `ModeStatus`
- Failure mode: provider unavailable or engine status error

### `assistant_send(request, on_event)`

- Purpose: submit a mode-aware prompt and stream progress
- Arguments: `AssistantSendRequestDto`, `Channel<AssistantStreamEvent>`
- Returns: `AssistantResponseDto`
- Failure mode: engine error, provider error, validation error, cancellation
- Phase 3 note: remains scaffold-only and is not used by active Code mode yet

### `assistant_cancel()`

- Purpose: cancel in-flight planning, tool execution, or generation
- Arguments: none
- Returns: void

### `mode_undo(mode)`

- Purpose: undo last provider-backed operation if supported
- Arguments: `{ mode: AppMode }`
- Returns: void
- Failure mode: unsupported mode, no undoable action, provider failure

### `mode_refresh_tools(mode)`

- Purpose: reconnect provider and refresh tool discovery
- Arguments: `{ mode: AppMode }`
- Returns: `ModeStatus`

## 12. Phase 2 Shell Bootstrap

Phase 2 startup behavior is:

1. Initialize the mode store.
2. Fetch `list_modes()` once.
3. Restore `activeMode` from `smolpc_unified_active_mode_v1`, defaulting to
   `code`.
4. Fetch `mode_status(activeMode)` for the active mode.
5. Start the existing engine bootstrap flow unchanged.
6. Auto-create one Code chat only if the unified chat storage is empty.

### Mode status loading

- `list_modes()` is fetched once at startup.
- `mode_status(mode)` is fetched lazily for the active mode at startup and on
  every mode switch.
- statuses may be cached by mode in the frontend store.
- Phase 2 does not require any backend contract changes for this.

## 13. Phase 2 Placeholder Behavior

Before provider integrations land:

- Code mode uses the current Codehelper behavior.
- GIMP, Blender, Writer, Calc, and Slides are visible in the shell.
- non-Code modes show mode-specific identity, suggestions, and provider status.
- non-Code modes keep the composer visible but disabled.
- the disabled composer reason is:
  `This mode is visible in the unified shell, but chat execution is not wired yet.`
- the shell must not fake Code-mode execution in other modes.

### Phase 3 Code-mode refinement

- Code-mode shell copy should feel like current Codehelper rather than a
  generic multi-mode placeholder.
- active Code-mode status should reflect real engine/backend/model state.
- no Phase 3 work should activate non-Code execution paths.

## 14. Migration Path

1. Preserve the current Codehelper shell as the shared shell.
2. Add mode state and per-mode histories.
3. Wrap current Codehelper behavior as Code mode.
4. Polish Code mode inside the unified shell without activating
   `assistant_send`.
5. Port GIMP behavior into a new GIMP provider.
6. Port Blender behavior into a new Blender provider.
7. Port LibreOffice behavior into one provider with Writer/Calc/Slides
   frontend configs.

The frontend should not import or embed standalone app code directly. It should
consume new unified stores, mode configs, and Tauri command contracts.
