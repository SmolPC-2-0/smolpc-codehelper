<script lang="ts">
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  type BootstrapEngineSnapshot = {
    healthy: boolean;
    protocol_version: string | null;
    engine_api_version: string | null;
    state: string | null;
    active_backend: string | null;
    active_model_id: string | null;
    runtime_engine: string | null;
    selection_reason: string | null;
    error: string | null;
  };

  type BootstrapStatus = {
    stage: string;
    notes: string[];
    engine: BootstrapEngineSnapshot;
  };

  type ModelDefinition = {
    id: string;
    name: string;
    size: string;
    disk_size_gb: number;
    min_ram_gb: number;
    directory: string;
    description: string;
  };

  type GenerationMetrics = {
    total_tokens: number;
    time_to_first_token_ms: number | null;
    tokens_per_second: number;
    total_time_ms: number;
  };

  type GenerationResult = {
    text: string;
    metrics: GenerationMetrics;
  };

  type LaneReadiness = {
    artifact_ready: boolean;
    bundle_ready: boolean;
    ready: boolean;
    reason: string;
  };

  type CheckModelResponse = {
    model_id: string;
    lanes: {
      openvino_npu: LaneReadiness;
      directml: LaneReadiness;
      cpu: LaneReadiness;
    };
  };

  type BackendStatus = {
    active_backend?: string | null;
    runtime_engine?: string | null;
    selection_state?: string | null;
    selection_reason?: string | null;
    available_backends?: string[];
  };

  type IntegrationIssueReport = {
    app_name: string;
    app_version: string;
    os: string;
    arch: string;
    hardware_summary: unknown;
    request_payload: unknown;
    http_status: number | null;
    response_body: string | null;
    engine_status: unknown;
    engine_meta: unknown;
    runtime_overrides: unknown;
  };

  type VerificationCheck = {
    id: string;
    ok: boolean;
    detail: string;
  };

  type RuntimeVerificationReport = {
    generated_at_unix: number;
    model_id: string;
    checks: VerificationCheck[];
    all_passed: boolean;
  };

  type EvidenceExportResult = {
    path: string;
    runtime_verification: RuntimeVerificationReport;
    integration_issue_report: IntegrationIssueReport;
  };

  let loadingBootstrap = $state(true);
  let actionBusy = $state(false);
  let commandError = $state<string | null>(null);
  let actionMessage = $state<string | null>(null);
  let bootstrap = $state<BootstrapStatus | null>(null);
  let models = $state<ModelDefinition[]>([]);
  let selectedModelId = $state('qwen3-4b-instruct-2507');
  let currentModelId = $state<string | null>(null);
  let readiness = $state<CheckModelResponse | null>(null);
  let backendStatus = $state<BackendStatus | null>(null);
  let prompt = $state('Draft a concise LibreOffice Writer paragraph about local-first AI for schools.');
  let generatedText = $state('');
  let streamingText = $state('');
  let lastMetrics = $state<GenerationMetrics | null>(null);
  let streaming = $state(false);
  let issueRequestPayload = $state('{}');
  let issueHttpStatus = $state('');
  let issueResponseBody = $state('');
  let integrationIssueReport = $state<IntegrationIssueReport | null>(null);
  let runtimeVerification = $state<RuntimeVerificationReport | null>(null);
  let evidenceExportPath = $state<string | null>(null);

  function formatError(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function normalizeEngineError(raw: string): string {
    if (raw.includes('HTTP 429')) {
      return 'Engine queue is full (HTTP 429). Retry after a short delay.';
    }
    if (raw.includes('HTTP 504')) {
      return 'Engine queue timed out (HTTP 504). Retry the request.';
    }
    if (raw.includes('INFERENCE_GENERATION_CANCELLED')) {
      return 'Generation was cancelled.';
    }
    if (raw.toLowerCase().includes('protocol') && raw.toLowerCase().includes('mismatch')) {
      return `Protocol mismatch detected: ${raw}`;
    }
    return raw;
  }

  function clearFeedback(): void {
    commandError = null;
    actionMessage = null;
  }

  async function refreshBootstrapStatus(): Promise<void> {
    loadingBootstrap = true;
    try {
      bootstrap = await invoke<BootstrapStatus>('get_bootstrap_status');
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      loadingBootstrap = false;
    }
  }

  async function ensureEngineStarted(): Promise<void> {
    actionBusy = true;
    clearFeedback();
    try {
      bootstrap = await invoke<BootstrapStatus>('ensure_engine_started');
      actionMessage = 'Engine ensure-started completed.';
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function refreshModels(): Promise<void> {
    try {
      models = await invoke<ModelDefinition[]>('list_models');
      if (!models.some((model) => model.id === selectedModelId) && models.length > 0) {
        selectedModelId = models[0].id;
      }
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    }
  }

  async function refreshCurrentModel(): Promise<void> {
    try {
      currentModelId = await invoke<string | null>('get_current_model');
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    }
  }

  async function refreshBackendStatus(): Promise<void> {
    try {
      backendStatus = await invoke<BackendStatus>('get_inference_backend_status');
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    }
  }

  async function refreshReadiness(): Promise<void> {
    if (!selectedModelId) {
      readiness = null;
      return;
    }
    try {
      readiness = await invoke<CheckModelResponse>('check_model_readiness', {
        modelId: selectedModelId
      });
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    }
  }

  async function loadSelectedModel(): Promise<void> {
    if (!selectedModelId.trim()) {
      commandError = 'Select a model before loading.';
      return;
    }

    actionBusy = true;
    clearFeedback();
    try {
      actionMessage = await invoke<string>('load_model', { modelId: selectedModelId });
      await Promise.all([
        refreshBootstrapStatus(),
        refreshCurrentModel(),
        refreshBackendStatus(),
        refreshReadiness()
      ]);
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function unloadCurrentModel(): Promise<void> {
    actionBusy = true;
    clearFeedback();
    try {
      actionMessage = await invoke<string>('unload_model');
      await Promise.all([
        refreshBootstrapStatus(),
        refreshCurrentModel(),
        refreshBackendStatus(),
        refreshReadiness()
      ]);
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function generateNonStream(): Promise<void> {
    if (!prompt.trim()) {
      commandError = 'Enter a prompt before generating.';
      return;
    }

    actionBusy = true;
    clearFeedback();
    streaming = false;
    streamingText = '';
    lastMetrics = null;

    try {
      const result = await invoke<GenerationResult>('generate_text', { prompt });
      generatedText = result.text;
      lastMetrics = result.metrics;
      actionMessage = 'Non-stream generation completed.';
      await Promise.all([refreshBootstrapStatus(), refreshBackendStatus()]);
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function generateStream(): Promise<void> {
    if (!prompt.trim()) {
      commandError = 'Enter a prompt before streaming.';
      return;
    }

    actionBusy = true;
    streaming = true;
    clearFeedback();
    generatedText = '';
    streamingText = '';
    lastMetrics = null;

    const onTokenChannel = new Channel<string>();
    onTokenChannel.onmessage = (token) => {
      streamingText += token;
    };

    try {
      const metrics = await invoke<GenerationMetrics>('inference_generate', {
        prompt,
        onToken: onTokenChannel
      });
      generatedText = streamingText;
      lastMetrics = metrics;
      actionMessage = 'Streaming generation completed.';
    } catch (error) {
      const message = normalizeEngineError(formatError(error));
      if (message.toLowerCase().includes('cancel')) {
        actionMessage = 'Generation cancelled.';
      } else {
        commandError = message;
      }
    } finally {
      streaming = false;
      actionBusy = false;
      await Promise.all([refreshBootstrapStatus(), refreshBackendStatus(), refreshCurrentModel()]);
    }
  }

  async function cancelGeneration(): Promise<void> {
    clearFeedback();
    try {
      await invoke('inference_cancel');
      actionMessage = 'Cancel signal sent to engine.';
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      streaming = false;
      await Promise.all([refreshBootstrapStatus(), refreshBackendStatus()]);
    }
  }

  function parseIssueContext():
    | { requestPayload: unknown; httpStatus: number | null; responseBody: string | null }
    | null {
    const requestPayload = issueRequestPayload.trim()
      ? (JSON.parse(issueRequestPayload) as unknown)
      : null;
    const parsedStatus =
      issueHttpStatus.trim() === '' ? null : Number.parseInt(issueHttpStatus, 10);
    if (parsedStatus !== null && Number.isNaN(parsedStatus)) {
      commandError = 'HTTP status must be an integer.';
      return null;
    }

    return {
      requestPayload,
      httpStatus: parsedStatus,
      responseBody: issueResponseBody.trim() || null
    };
  }

  async function createIssueReport(): Promise<void> {
    actionBusy = true;
    clearFeedback();
    try {
      const context = parseIssueContext();
      if (!context) {
        return;
      }

      integrationIssueReport = await invoke<IntegrationIssueReport>('create_integration_issue_report', {
        requestPayload: context.requestPayload,
        httpStatus: context.httpStatus,
        responseBody: context.responseBody
      });
      evidenceExportPath = null;
      actionMessage = 'Integration issue report snapshot generated.';
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function copyIssueReport(): Promise<void> {
    if (!integrationIssueReport) {
      commandError = 'Generate an issue report first.';
      return;
    }

    clearFeedback();
    try {
      await navigator.clipboard.writeText(JSON.stringify(integrationIssueReport, null, 2));
      actionMessage = 'Issue report JSON copied to clipboard.';
    } catch (error) {
      commandError = `Clipboard copy failed: ${formatError(error)}`;
    }
  }

  async function exportEvidenceBundle(): Promise<void> {
    if (!selectedModelId.trim()) {
      commandError = 'Select a model before exporting an evidence bundle.';
      return;
    }

    actionBusy = true;
    clearFeedback();
    try {
      const context = parseIssueContext();
      if (!context) {
        return;
      }

      const result = await invoke<EvidenceExportResult>('export_phase1_evidence_bundle', {
        modelId: selectedModelId,
        requestPayload: context.requestPayload,
        httpStatus: context.httpStatus,
        responseBody: context.responseBody
      });
      runtimeVerification = result.runtime_verification;
      integrationIssueReport = result.integration_issue_report;
      evidenceExportPath = result.path;
      actionMessage = 'Phase 1 evidence bundle exported.';
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function runRuntimeChecklist(): Promise<void> {
    if (!selectedModelId.trim()) {
      commandError = 'Select a model before running runtime verification.';
      return;
    }

    actionBusy = true;
    clearFeedback();
    try {
      runtimeVerification = await invoke<RuntimeVerificationReport>('run_runtime_verification_checklist', {
        modelId: selectedModelId
      });
      actionMessage = runtimeVerification.all_passed
        ? 'Runtime verification checks passed.'
        : 'Runtime verification returned one or more failing checks.';
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  onMount(async () => {
    clearFeedback();
    await refreshBootstrapStatus();
    await refreshModels();
    await Promise.all([refreshCurrentModel(), refreshBackendStatus()]);
    await refreshReadiness();
  });
</script>

<main class="container">
  <h1>SmolPC LibreOffice Assistant</h1>
  <p class="subtitle">Phase 1: shared-engine onboarding flow</p>

  <div class="actions">
    <button
      type="button"
      onclick={() => void refreshBootstrapStatus()}
      disabled={loadingBootstrap || actionBusy}
    >
      {loadingBootstrap ? 'Refreshing...' : 'Refresh Bootstrap'}
    </button>
    <button type="button" onclick={() => void ensureEngineStarted()} disabled={actionBusy || loadingBootstrap}>
      {actionBusy ? 'Working...' : 'Ensure Engine Started'}
    </button>
    <button type="button" onclick={() => void refreshReadiness()} disabled={actionBusy}>
      Refresh Readiness
    </button>
  </div>

  {#if commandError}
    <p class="error">{commandError}</p>
  {/if}
  {#if actionMessage}
    <p class="ok">{actionMessage}</p>
  {/if}

  {#if bootstrap}
    <section class="panel">
      <h2>Bootstrap Snapshot</h2>
      <dl>
        <div><dt>stage</dt><dd>{bootstrap.stage}</dd></div>
        <div><dt>healthy</dt><dd>{bootstrap.engine.healthy ? 'true' : 'false'}</dd></div>
        <div><dt>state</dt><dd>{bootstrap.engine.state ?? 'unknown'}</dd></div>
        <div><dt>active_backend</dt><dd>{bootstrap.engine.active_backend ?? 'none'}</dd></div>
        <div><dt>runtime_engine</dt><dd>{bootstrap.engine.runtime_engine ?? 'none'}</dd></div>
        <div><dt>selection_reason</dt><dd>{bootstrap.engine.selection_reason ?? 'none'}</dd></div>
        <div><dt>active_model_id</dt><dd>{bootstrap.engine.active_model_id ?? 'none'}</dd></div>
        <div><dt>engine_api_version</dt><dd>{bootstrap.engine.engine_api_version ?? 'unknown'}</dd></div>
        <div><dt>protocol_version</dt><dd>{bootstrap.engine.protocol_version ?? 'unknown'}</dd></div>
      </dl>
      {#if bootstrap.engine.error}
        <p class="error">{bootstrap.engine.error}</p>
      {/if}
    </section>

    <section class="panel">
      <h2>Phase Notes</h2>
      <ul>
        {#each bootstrap.notes as note}
          <li>{note}</li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="panel">
    <h2>Model Control</h2>
    <div class="row">
      <label for="model-id">Model</label>
      <select id="model-id" bind:value={selectedModelId} disabled={actionBusy}>
        {#each models as model}
          <option value={model.id}>{model.name} ({model.id})</option>
        {/each}
      </select>
    </div>
    <div class="actions">
      <button type="button" onclick={() => void refreshModels()} disabled={actionBusy}>Refresh Models</button>
      <button type="button" onclick={() => void loadSelectedModel()} disabled={actionBusy || !selectedModelId}>
        Load Model
      </button>
      <button type="button" onclick={() => void unloadCurrentModel()} disabled={actionBusy}>Unload Model</button>
    </div>
    <p class="kv">Current model: <code>{currentModelId ?? 'none'}</code></p>
  </section>

  <section class="panel">
    <h2>Generation</h2>
    <div class="row stacked">
      <label for="prompt">Prompt</label>
      <textarea id="prompt" bind:value={prompt} rows="5" disabled={actionBusy && !streaming}></textarea>
    </div>
    <div class="actions">
      <button type="button" onclick={() => void generateNonStream()} disabled={actionBusy || !prompt.trim()}>
        Generate (Non-Stream)
      </button>
      <button type="button" onclick={() => void generateStream()} disabled={actionBusy || !prompt.trim()}>
        Generate (Stream)
      </button>
      <button type="button" onclick={() => void cancelGeneration()} disabled={!streaming}>
        Cancel
      </button>
    </div>
    <div class="output-grid">
      <div>
        <h3>Output</h3>
        <pre>{generatedText || '(none yet)'}</pre>
      </div>
      <div>
        <h3>Streaming Buffer</h3>
        <pre>{streamingText || '(no stream chunks yet)'}</pre>
      </div>
    </div>
    {#if lastMetrics}
      <p class="kv">
        Metrics:
        <code>
          tokens={lastMetrics.total_tokens},
          ttft_ms={lastMetrics.time_to_first_token_ms ?? 'n/a'},
          tps={lastMetrics.tokens_per_second.toFixed(2)},
          total_ms={lastMetrics.total_time_ms}
        </code>
      </p>
    {/if}
  </section>

  <section class="panel">
    <h2>Readiness</h2>
    {#if readiness}
      <div class="lane-grid">
        <div>
          <h3>openvino_npu</h3>
          <p><code>ready={readiness.lanes.openvino_npu.ready ? 'true' : 'false'}</code></p>
          <p><code>reason={readiness.lanes.openvino_npu.reason}</code></p>
        </div>
        <div>
          <h3>directml</h3>
          <p><code>ready={readiness.lanes.directml.ready ? 'true' : 'false'}</code></p>
          <p><code>reason={readiness.lanes.directml.reason}</code></p>
        </div>
        <div>
          <h3>cpu</h3>
          <p><code>ready={readiness.lanes.cpu.ready ? 'true' : 'false'}</code></p>
          <p><code>reason={readiness.lanes.cpu.reason}</code></p>
        </div>
      </div>
    {:else}
      <p class="muted">No readiness snapshot yet.</p>
    {/if}
  </section>

  <section class="panel">
    <h2>Backend Status</h2>
    {#if backendStatus}
      <p class="kv">active_backend: <code>{backendStatus.active_backend ?? 'none'}</code></p>
      <p class="kv">runtime_engine: <code>{backendStatus.runtime_engine ?? 'none'}</code></p>
      <p class="kv">selection_state: <code>{backendStatus.selection_state ?? 'none'}</code></p>
      <p class="kv">selection_reason: <code>{backendStatus.selection_reason ?? 'none'}</code></p>
      <p class="kv">
        available_backends:
        <code>{(backendStatus.available_backends ?? []).join(', ') || 'none'}</code>
      </p>
    {:else}
      <p class="muted">No backend status snapshot yet.</p>
    {/if}
  </section>

  <section class="panel">
    <h2>Runtime Verification</h2>
    <p class="muted">
      Runs contract-level checks aligned with `docs/APP_ONBOARDING_PLAYBOOK.md` against the selected
      model.
    </p>
    <div class="actions">
      <button type="button" onclick={() => void runRuntimeChecklist()} disabled={actionBusy || !selectedModelId}>
        Run Verification Checklist
      </button>
    </div>
    {#if runtimeVerification}
      <p class="kv">
        result:
        <code>{runtimeVerification.all_passed ? 'all_passed=true' : 'all_passed=false'}</code>
      </p>
      <div class="check-grid">
        {#each runtimeVerification.checks as check}
          <div class={check.ok ? 'check check-ok' : 'check check-fail'}>
            <p><code>{check.id}</code></p>
            <p>{check.detail}</p>
          </div>
        {/each}
      </div>
    {:else}
      <p class="muted">No runtime verification report yet.</p>
    {/if}
  </section>

  <section class="panel">
    <h2>Integration Issue Report</h2>
    <p class="muted">
      Generates the onboarding issue payload with app/version, OS/hardware summary, request/response
      payload, engine status/meta snapshots, and runtime override flags.
    </p>
    <div class="row stacked">
      <label for="issue-request-payload">Request Payload (JSON)</label>
      <textarea
        id="issue-request-payload"
        bind:value={issueRequestPayload}
        rows="5"
        disabled={actionBusy}
      ></textarea>
    </div>
    <div class="row">
      <label for="issue-http-status">HTTP Status</label>
      <input
        id="issue-http-status"
        type="text"
        bind:value={issueHttpStatus}
        placeholder="e.g. 429"
        disabled={actionBusy}
      />
    </div>
    <div class="row stacked">
      <label for="issue-response-body">Response Body (text)</label>
      <textarea
        id="issue-response-body"
        bind:value={issueResponseBody}
        rows="4"
        disabled={actionBusy}
      ></textarea>
    </div>
    <div class="actions">
      <button type="button" onclick={() => void createIssueReport()} disabled={actionBusy}>
        Generate Issue Report
      </button>
      <button type="button" onclick={() => void exportEvidenceBundle()} disabled={actionBusy || !selectedModelId}>
        Export Evidence Bundle
      </button>
      <button type="button" onclick={() => void copyIssueReport()} disabled={actionBusy || !integrationIssueReport}>
        Copy JSON
      </button>
    </div>
    {#if evidenceExportPath}
      <p class="kv">Evidence file: <code>{evidenceExportPath}</code></p>
    {/if}
    {#if integrationIssueReport}
      <pre>{JSON.stringify(integrationIssueReport, null, 2)}</pre>
    {:else}
      <p class="muted">No issue report generated yet.</p>
    {/if}
  </section>
</main>

<style>
  .container {
    max-width: 820px;
    margin: 7vh auto;
    background: #ffffff;
    border: 1px solid #d1d5db;
    border-radius: 12px;
    padding: 2rem;
  }

  h1 {
    margin: 0;
    font-size: 1.8rem;
    color: #111827;
  }

  .subtitle {
    margin-top: 0.5rem;
    color: #374151;
  }

  .actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 1.25rem;
    margin-bottom: 1.25rem;
    flex-wrap: wrap;
  }

  button {
    border: 1px solid #0f172a;
    border-radius: 8px;
    background: #0f172a;
    color: #ffffff;
    padding: 0.55rem 0.9rem;
    cursor: pointer;
    font-weight: 600;
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .panel {
    border: 1px solid #e5e7eb;
    border-radius: 10px;
    padding: 1rem;
    margin-top: 1rem;
    background: #fafafa;
  }

  h2 {
    margin-top: 0;
    margin-bottom: 0.75rem;
    font-size: 1rem;
    color: #111827;
  }

  h3 {
    margin-top: 0;
    margin-bottom: 0.4rem;
    font-size: 0.9rem;
    color: #111827;
  }

  dl {
    margin: 0;
  }

  dl div {
    display: grid;
    grid-template-columns: 180px 1fr;
    gap: 0.5rem;
    margin-bottom: 0.3rem;
  }

  dt {
    font-family: 'Consolas', 'SFMono-Regular', Menlo, monospace;
    color: #334155;
  }

  dd {
    margin: 0;
    color: #111827;
    word-break: break-word;
  }

  ul {
    margin: 0;
    padding-left: 1.25rem;
    color: #1f2937;
  }

  li + li {
    margin-top: 0.35rem;
  }

  .error {
    margin-top: 0.75rem;
    color: #b91c1c;
    font-weight: 600;
    word-break: break-word;
  }

  .ok {
    margin-top: 0.75rem;
    color: #166534;
    font-weight: 600;
  }

  .row {
    display: grid;
    grid-template-columns: 130px 1fr;
    gap: 0.75rem;
    align-items: center;
    margin-bottom: 0.75rem;
  }

  .row.stacked {
    grid-template-columns: 1fr;
  }

  label {
    color: #1f2937;
    font-weight: 600;
  }

  select,
  input,
  textarea {
    width: 100%;
    border: 1px solid #cbd5e1;
    border-radius: 8px;
    padding: 0.55rem 0.7rem;
    background: #ffffff;
    color: #111827;
    font: inherit;
  }

  textarea {
    resize: vertical;
    min-height: 120px;
  }

  .output-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.9rem;
    margin-top: 1rem;
  }

  pre {
    margin: 0;
    padding: 0.75rem;
    border-radius: 8px;
    border: 1px solid #e2e8f0;
    background: #f8fafc;
    color: #0f172a;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 280px;
    overflow: auto;
    font-family: 'Consolas', 'SFMono-Regular', Menlo, monospace;
    font-size: 0.85rem;
    line-height: 1.35;
  }

  .lane-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.75rem;
  }

  .lane-grid > div {
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    background: #ffffff;
    padding: 0.7rem;
  }

  .kv {
    margin: 0.45rem 0;
    color: #1f2937;
  }

  .muted {
    margin: 0;
    color: #6b7280;
  }

  code {
    font-family: 'Consolas', 'SFMono-Regular', Menlo, monospace;
    font-size: 0.85rem;
  }

  .check-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.75rem;
  }

  .check {
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    padding: 0.7rem;
    background: #ffffff;
  }

  .check p {
    margin: 0.25rem 0;
    color: #1f2937;
  }

  .check-ok {
    border-color: #86efac;
    background: #f0fdf4;
  }

  .check-fail {
    border-color: #fca5a5;
    background: #fef2f2;
  }

  @media (max-width: 640px) {
    .container {
      margin: 0;
      min-height: 100vh;
      border-radius: 0;
      border-left: 0;
      border-right: 0;
      padding: 1.25rem;
    }

    dl div {
      grid-template-columns: 1fr;
      gap: 0.1rem;
    }

    .row {
      grid-template-columns: 1fr;
      gap: 0.3rem;
    }

    .output-grid {
      grid-template-columns: 1fr;
    }

    .lane-grid {
      grid-template-columns: 1fr;
    }

    .check-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
