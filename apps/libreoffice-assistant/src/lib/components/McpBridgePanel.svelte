<script lang="ts">
  import type { McpStatus, McpTool, ToolResult } from '../types/libreoffice';

  type Props = {
    actionBusy: boolean;
    mcpStatus: McpStatus | null;
    mcpTools: McpTool[];
    selectedMcpTool: string;
    mcpArguments: string;
    mcpToolResult: ToolResult | null;
    onRefreshMcpStatus: () => void;
    onStartMcpServer: () => void;
    onStopMcpServer: () => void;
    onLoadMcpTools: () => void;
    onCallSelectedMcpTool: () => void;
    onSelectedMcpToolChange: (toolName: string) => void;
    onMcpArgumentsChange: (nextValue: string) => void;
    onApplyToolArgumentTemplate: (toolName: string) => void;
  };

  let {
    actionBusy,
    mcpStatus,
    mcpTools,
    selectedMcpTool,
    mcpArguments,
    mcpToolResult,
    onRefreshMcpStatus,
    onStartMcpServer,
    onStopMcpServer,
    onLoadMcpTools,
    onCallSelectedMcpTool,
    onSelectedMcpToolChange,
    onMcpArgumentsChange,
    onApplyToolArgumentTemplate
  }: Props = $props();

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
</script>

<section class="panel">
  <h2>MCP Bridge</h2>
  <p class="muted">
    Phase 2 diagnostics for LibreOffice MCP runtime startup and tool invocation.
  </p>
  <div class="actions">
    <button type="button" onclick={onRefreshMcpStatus} disabled={actionBusy}>Refresh MCP Status</button>
    <button type="button" onclick={onStartMcpServer} disabled={actionBusy || mcpStatus?.running}>
      Start MCP Server
    </button>
    <button type="button" onclick={onStopMcpServer} disabled={actionBusy || !mcpStatus?.running}>
      Stop MCP Server
    </button>
    <button type="button" onclick={onLoadMcpTools} disabled={actionBusy || !mcpStatus?.running}>
      Refresh MCP Tools
    </button>
  </div>
  <p class="kv">running: <code>{mcpStatus?.running ? 'true' : 'false'}</code></p>
  <p class="kv">error: <code>{mcpStatus?.error_message ?? 'none'}</code></p>
  <p class="kv">tools_loaded: <code>{mcpTools.length}</code></p>
  <div class="row">
    <label for="mcp-tool">Tool</label>
    <select
      id="mcp-tool"
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
  <div class="row stacked">
    <label for="mcp-args">Tool Arguments (JSON)</label>
    <textarea
      id="mcp-args"
      value={mcpArguments}
      rows="4"
      disabled={actionBusy}
      oninput={handleArgumentsInput}
    ></textarea>
  </div>
  <div class="actions">
    <button
      type="button"
      onclick={onCallSelectedMcpTool}
      disabled={actionBusy || !mcpStatus?.running || !selectedMcpTool}
    >
      Invoke MCP Tool
    </button>
  </div>
  {#if mcpToolResult}
    <pre>{JSON.stringify(mcpToolResult, null, 2)}</pre>
  {:else}
    <p class="muted">No MCP tool result yet.</p>
  {/if}
</section>
