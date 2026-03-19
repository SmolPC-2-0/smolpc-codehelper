<script lang="ts">
  import type { McpStatus, McpTool, ToolResult } from '../types/libreoffice';
  import type { SourceParityWorkflowMode } from '../types/sourceParity';

  interface Props {
    actionBusy: boolean;
    actionMessage: string | null;
    commandError: string | null;
    mcpStatus: McpStatus | null;
    mcpTools: McpTool[];
    selectedMcpTool: string;
    mcpArguments: string;
    mcpToolResult: ToolResult | null;
    workflowMode: SourceParityWorkflowMode;
    onRefreshMcpStatus: () => void;
    onStartMcpServer: () => void;
    onStopMcpServer: () => void;
    onLoadMcpTools: () => void;
    onCallSelectedMcpTool: () => void;
    onSelectedMcpToolChange: (toolName: string) => void;
    onMcpArgumentsChange: (nextValue: string) => void;
    onApplyToolArgumentTemplate: (toolName: string) => void;
  }

  let {
    actionBusy,
    actionMessage,
    commandError,
    mcpStatus,
    mcpTools,
    selectedMcpTool,
    mcpArguments,
    mcpToolResult,
    workflowMode,
    onRefreshMcpStatus,
    onStartMcpServer,
    onStopMcpServer,
    onLoadMcpTools,
    onCallSelectedMcpTool,
    onSelectedMcpToolChange,
    onMcpArgumentsChange,
    onApplyToolArgumentTemplate
  }: Props = $props();

  const selectedTool = $derived(
    mcpTools.find((tool) => tool.name === selectedMcpTool) ?? null
  );

  function handleToolChange(event: Event): void {
    const toolName = (event.currentTarget as HTMLSelectElement | null)?.value ?? '';
    onSelectedMcpToolChange(toolName);
    if (toolName) {
      onApplyToolArgumentTemplate(toolName);
    }
  }

  function handleArgumentsInput(event: Event): void {
    const nextValue = (event.currentTarget as HTMLTextAreaElement | null)?.value ?? '';
    onMcpArgumentsChange(nextValue);
  }

  function validateMcpArguments(value: string): string | null {
    const trimmed = value.trim();
    if (!trimmed) {
      return null;
    }

    try {
      const parsed = JSON.parse(trimmed) as unknown;
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        return 'Arguments must be a JSON object.';
      }
      return null;
    } catch {
      return 'Arguments must be valid JSON.';
    }
  }

  const argumentsError = $derived(validateMcpArguments(mcpArguments));
</script>

<div class="tools-workspace">
  <div class="tools-summary">
    <p class="kv">
      MCP status:
      <code>{mcpStatus?.running ? 'running' : 'stopped'}</code>
    </p>
    <p class="kv">
      tools_loaded:
      <code>{mcpTools.length}</code>
    </p>
    <p class="kv">
      workflow_mode:
      <code>{workflowMode}</code>
    </p>
    {#if workflowMode === 'tool_first'}
      <p class="tool-first-note">
        Chat in tool-first mode reuses this selected tool and JSON arguments.
      </p>
    {/if}
    {#if mcpStatus?.error_message}
      <p class="error">MCP detail: {mcpStatus.error_message}</p>
    {/if}
  </div>

  <div class="actions">
    <button type="button" onclick={onRefreshMcpStatus} disabled={actionBusy}>Refresh MCP Status</button>
    <button type="button" onclick={onStartMcpServer} disabled={actionBusy || mcpStatus?.running}>
      Start MCP
    </button>
    <button type="button" onclick={onStopMcpServer} disabled={actionBusy || !mcpStatus?.running}>
      Stop MCP
    </button>
    <button type="button" onclick={onLoadMcpTools} disabled={actionBusy || !mcpStatus?.running}>
      Refresh Tools
    </button>
  </div>

  <div class="row">
    <label for="source-parity-mcp-tool">Tool</label>
    <select
      id="source-parity-mcp-tool"
      value={selectedMcpTool}
      disabled={actionBusy || mcpTools.length === 0}
      onchange={handleToolChange}
    >
      {#if mcpTools.length === 0}
        <option value="">(no tools loaded)</option>
      {:else}
        {#each mcpTools as tool}
          <option value={tool.name}>{tool.name}</option>
        {/each}
      {/if}
    </select>
  </div>

  {#if selectedTool}
    <div class="tool-details">
      <h3>{selectedTool.name}</h3>
      <p>{selectedTool.description || 'No tool description provided.'}</p>
    </div>
  {/if}

  <div class="row stacked">
    <label for="source-parity-mcp-args">Tool Arguments (JSON object)</label>
    <textarea
      id="source-parity-mcp-args"
      rows="8"
      value={mcpArguments}
      disabled={actionBusy}
      oninput={handleArgumentsInput}
    ></textarea>
    {#if argumentsError}
      <p class="error">{argumentsError}</p>
    {/if}
  </div>

  <div class="actions">
    <button
      type="button"
      class="secondary"
      onclick={() => selectedMcpTool && onApplyToolArgumentTemplate(selectedMcpTool)}
      disabled={actionBusy || !selectedMcpTool}
    >
      Apply Template
    </button>
    <button
      type="button"
      onclick={onCallSelectedMcpTool}
      disabled={actionBusy || !mcpStatus?.running || !selectedMcpTool || Boolean(argumentsError)}
    >
      Invoke Tool
    </button>
  </div>

  {#if selectedTool?.input_schema}
    <details>
      <summary>Input Schema</summary>
      <pre>{JSON.stringify(selectedTool.input_schema, null, 2)}</pre>
    </details>
  {/if}

  <div class="result">
    <h3>Tool Result</h3>
    {#if mcpToolResult}
      <pre>{JSON.stringify(mcpToolResult, null, 2)}</pre>
    {:else}
      <p class="muted">No MCP tool result yet.</p>
    {/if}
  </div>

  {#if commandError}
    <p class="error">{commandError}</p>
  {:else if actionMessage}
    <p class="ok">{actionMessage}</p>
  {/if}
</div>

<style>
  .tools-workspace {
    display: grid;
    gap: 0.85rem;
    border: 1px solid #334155;
    border-radius: 10px;
    background: #020617;
    padding: 0.95rem;
  }

  .tools-summary {
    border: 1px solid #1e293b;
    border-radius: 8px;
    background: #0f172a;
    padding: 0.75rem;
  }

  .tool-first-note {
    margin: 0.5rem 0 0;
    color: #bfdbfe;
  }

  .row label {
    color: #cbd5e1;
  }

  textarea {
    min-height: 150px;
    border: 1px solid #334155;
    border-radius: 8px;
    background: #0b1220;
    color: #e2e8f0;
  }

  select {
    border: 1px solid #334155;
    border-radius: 8px;
    background: #0b1220;
    color: #e2e8f0;
  }

  .tool-details {
    border: 1px solid #1e293b;
    border-radius: 8px;
    background: #0f172a;
    padding: 0.7rem 0.75rem;
  }

  .tool-details h3 {
    margin-bottom: 0.35rem;
    color: #7dd3fc;
  }

  .tool-details p {
    margin: 0;
    color: #cbd5e1;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .result {
    border: 1px solid #1e293b;
    border-radius: 8px;
    background: #0f172a;
    padding: 0.75rem;
  }

  .result h3 {
    margin-bottom: 0.6rem;
    color: #7dd3fc;
  }

  details {
    border: 1px solid #1e293b;
    border-radius: 8px;
    background: #0f172a;
    padding: 0.6rem 0.75rem;
  }

  details summary {
    cursor: pointer;
    color: #cbd5e1;
    font-weight: 700;
  }

  details pre {
    margin-top: 0.6rem;
    background: #020617;
    border-color: #334155;
    color: #e2e8f0;
    max-height: 240px;
  }

  .muted {
    color: #94a3b8;
  }
</style>
