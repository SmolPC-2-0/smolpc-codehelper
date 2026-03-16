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
- mode dropdown
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
    ModeDropdown
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

## 7. Per-Mode Histories

Chat history is stored in one shared store but filtered by `Chat.mode`.

Rules:

1. Every chat is tagged with exactly one mode.
2. The sidebar shows only chats matching the active mode.
3. Switching modes does not delete or migrate chats.
4. Storage keys must be versioned for the unified product so current standalone
   Codehelper storage is not silently reused.

## 8. Capability Flags

Capability flags control which existing Codehelper surfaces stay visible in each
mode.

### Shared across all modes

- sidebar / chat history
- conversation view
- composer
- status bar
- model info panel
- hardware panel

### Code-only in v1

- benchmark panel
- code-oriented workspace controls
- code-specific quick actions

### Mode-specific

- undo button only where the mode supports it
- tool summary surfaces only where the provider supports tools

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

## 12. Migration Path

1. Preserve the current Codehelper shell as the shared shell.
2. Add mode state and per-mode histories.
3. Wrap current Codehelper behavior as Code mode.
4. Port GIMP behavior into a new GIMP provider.
5. Port Blender behavior into a new Blender provider.
6. Port LibreOffice behavior into one provider with Writer/Calc/Slides
   frontend configs.

The frontend should not import or embed standalone app code directly. It should
consume new unified stores, mode configs, and Tauri command contracts.
