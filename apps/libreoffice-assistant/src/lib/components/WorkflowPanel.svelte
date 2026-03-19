<script lang="ts">
  import type { WorkflowOutcomeTag } from '../types/libreoffice';

  type Props = {
    actionBusy: boolean;
    workflowPrompt: string;
    selectedModelId: string;
    selectedMcpTool: string;
    workflowDepthUsed: number;
    workflowToolCallsUsed: number;
    workflowOutcome: WorkflowOutcomeTag;
    workflowErrorDetail: string | null;
    workflowFinalResponse: string;
    workflowTrace: string[];
    workflowLastEvidenceFile: string | null;
    hasWorkflowEvidence: boolean;
    onWorkflowPromptChange: (nextValue: string) => void;
    onRunMcpAssistedWorkflow: () => void;
    onRunToolFirstWorkflow: () => void;
    onCopyWorkflowEvidence: () => void;
    onExportWorkflowEvidence: () => void;
  };

  let {
    actionBusy,
    workflowPrompt,
    selectedModelId,
    selectedMcpTool,
    workflowDepthUsed,
    workflowToolCallsUsed,
    workflowOutcome,
    workflowErrorDetail,
    workflowFinalResponse,
    workflowTrace,
    workflowLastEvidenceFile,
    hasWorkflowEvidence,
    onWorkflowPromptChange,
    onRunMcpAssistedWorkflow,
    onRunToolFirstWorkflow,
    onCopyWorkflowEvidence,
    onExportWorkflowEvidence
  }: Props = $props();

  function handleWorkflowPromptInput(event: Event): void {
    const nextValue = (event.currentTarget as HTMLTextAreaElement | null)?.value ?? '';
    onWorkflowPromptChange(nextValue);
  }
</script>

<section class="panel">
  <h2>Phase 3 Workflow</h2>
  <p class="muted">
    Runs model-to-MCP orchestration with strict preflight checks, one-retry recovery rules, and outcome tags.
  </p>
  <p class="muted">
    Fast-mode limits: <code>max_length=64</code>, per-turn timeout <code>45s</code>.
  </p>
  <p class="muted">
    On slow machines use <code>Run Tool-First Fast Path</code>: it executes MCP first, then requests one short summary turn.
  </p>
  <div class="row stacked">
    <label for="workflow-prompt">Workflow Prompt</label>
    <textarea
      id="workflow-prompt"
      value={workflowPrompt}
      rows="4"
      disabled={actionBusy}
      oninput={handleWorkflowPromptInput}
    ></textarea>
  </div>
  <div class="actions">
    <button
      type="button"
      onclick={onRunMcpAssistedWorkflow}
      disabled={actionBusy || !workflowPrompt.trim() || !selectedModelId}
    >
      Run MCP-Assisted Flow
    </button>
    <button
      type="button"
      onclick={onRunToolFirstWorkflow}
      disabled={actionBusy || !workflowPrompt.trim() || !selectedMcpTool || !selectedModelId}
    >
      Run Tool-First Fast Path
    </button>
  </div>
  <p class="kv">
    workflow_stats:
    <code>depth={workflowDepthUsed}, tool_calls={workflowToolCallsUsed}</code>
  </p>
  <p class="kv">
    workflow_outcome:
    <code>{workflowOutcome}</code>
  </p>
  {#if workflowErrorDetail}
    <p class="error">{workflowErrorDetail}</p>
  {/if}
  <div class="output-grid">
    <div>
      <h3>Workflow Final Response</h3>
      <pre>{workflowFinalResponse || '(no workflow response yet)'}</pre>
    </div>
    <div>
      <h3>Workflow Trace</h3>
      <pre>{workflowTrace.length > 0 ? workflowTrace.join('\n') : '(no workflow trace yet)'}</pre>
    </div>
  </div>
  <div class="actions">
    <button type="button" onclick={onCopyWorkflowEvidence} disabled={actionBusy || !hasWorkflowEvidence}>
      Copy Workflow Evidence
    </button>
    <button type="button" onclick={onExportWorkflowEvidence} disabled={actionBusy || !hasWorkflowEvidence}>
      Export Workflow Evidence
    </button>
  </div>
  {#if workflowLastEvidenceFile}
    <p class="kv">Workflow evidence file: <code>{workflowLastEvidenceFile}</code></p>
  {/if}
</section>
