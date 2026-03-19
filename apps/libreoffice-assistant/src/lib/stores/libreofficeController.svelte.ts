  import { Channel, invoke } from '@tauri-apps/api/core';
  import type {
    BackendStatus,
    BootstrapStatus,
    ChatTurn,
    CheckModelResponse,
    EvidenceExportResult,
    GenerationConfig,
    GenerationMetrics,
    GenerationResult,
    IntegrationIssueReport,
    McpStatus,
    McpTool,
    ModelDefinition,
    RuntimeVerificationReport,
    ToolResult,
    WorkflowOutcomeTag
  } from '../types/libreoffice';
  import {
    buildChatMlPrompt,
    buildLocalFallbackSummary,
    buildMcpArgsTemplate,
    buildWorkflowToolCatalogInstruction,
    extractToolCallsFromModelText,
    hasHelperConnectionError,
    hasToolExecutionError,
    summarizeToolResult
  } from '../utils/workflowHelpers';

  const PHASE3_MAX_TOOL_CHAIN_DEPTH = 4;
  const PHASE3_MAX_TOOL_CALLS_PER_RESPONSE = 1;
  const PHASE3_MODEL_TURN_TIMEOUT_MS = 45000;
  const MODEL_LOAD_TIMEOUT_MS = 120000;
  const CPU_SAFE_MODEL_ID = 'qwen2.5-coder-1.5b';
  const DEFAULT_WORKFLOW_TEMPERATURE = 0.0;
  const DEFAULT_WORKFLOW_MAX_TOKENS = 64;
  const DEFAULT_PYTHON_COMMAND = 'python';
  const PHASE3_SYSTEM_BASE_INSTRUCTION = `You are a LibreOffice assistant that can use MCP tools.
When you need to call a tool, respond with JSON only in this exact shape:
{"tool_call":{"name":"<tool_name>","arguments":{...}}}
Do not include markdown fences when returning tool_call JSON.
Use \`list_documents\` for document listing requests (not \`list_files\`).
If no tool call is needed, respond with the final user-facing answer in plain text.`;

  function buildPhase3SystemInstruction(): string {
    const customSystemPrompt = workflowSystemPrompt.trim();
    const customPromptSection = customSystemPrompt
      ? `\n\nAdditional operator instructions:\n${customSystemPrompt}`
      : '';
    return `${PHASE3_SYSTEM_BASE_INSTRUCTION}\n\n${buildWorkflowToolCatalogInstruction(mcpTools)}${customPromptSection}`;
  }

  function buildWorkflowGenerationConfig(): GenerationConfig {
    return {
      max_length: Math.max(16, Math.min(8192, Math.floor(workflowMaxTokens))),
      temperature: Math.max(0, Math.min(2, workflowTemperature)),
      top_k: 1,
      top_p: 1.0
    };
  }

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
  let mcpStatus = $state<McpStatus | null>(null);
  let mcpTools = $state<McpTool[]>([]);
  let selectedMcpTool = $state('');
  let mcpArguments = $state('{}');
  let mcpToolResult = $state<ToolResult | null>(null);
  let workflowPrompt = $state(
    'List LibreOffice text documents in my Documents folder, then summarize the result in one short paragraph.'
  );
  let workflowFinalResponse = $state('');
  let workflowTrace = $state<string[]>([]);
  let workflowDepthUsed = $state(0);
  let workflowToolCallsUsed = $state(0);
  let workflowOutcome = $state<WorkflowOutcomeTag>('none');
  let workflowErrorDetail = $state<string | null>(null);
  let workflowLastEvidenceFile = $state<string | null>(null);
  let workflowSystemPrompt = $state('');
  let workflowTemperature = $state(DEFAULT_WORKFLOW_TEMPERATURE);
  let workflowMaxTokens = $state(DEFAULT_WORKFLOW_MAX_TOKENS);
  let mcpPythonPath = $state(DEFAULT_PYTHON_COMMAND);

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

  function withTimeout<T>(
    promise: Promise<T>,
    timeoutMs: number,
    timeoutMessage: string,
    onTimeout?: () => Promise<void>
  ): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      let settled = false;
      const timeoutId = setTimeout(() => {
        if (settled) {
          return;
        }
        settled = true;
        void (async () => {
          if (onTimeout) {
            try {
              await onTimeout();
            } catch (error) {
              console.warn('Timeout cancellation callback failed:', error);
            }
          }
        })();
        reject(new Error(timeoutMessage));
      }, timeoutMs);

      promise
        .then((value) => {
          if (settled) {
            return;
          }
          settled = true;
          clearTimeout(timeoutId);
          resolve(value);
        })
        .catch((error: unknown) => {
          if (settled) {
            return;
          }
          settled = true;
          clearTimeout(timeoutId);
          reject(error);
        });
    });
  }

  async function cancelGenerationAfterTimeout(): Promise<void> {
    try {
      await invoke('inference_cancel');
    } catch {
      // Best effort cancellation for timed-out generation calls.
    }
  }

  function parseJsonInput(value: string, fieldLabel: string): unknown | null {
    try {
      return JSON.parse(value) as unknown;
    } catch {
      commandError = `${fieldLabel} must contain valid JSON.`;
      return null;
    }
  }

  function parseJsonObjectInput(value: string, fieldLabel: string): Record<string, unknown> | null {
    const parsed = parseJsonInput(value, fieldLabel);
    if (parsed === null) {
      return null;
    }
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      commandError = `${fieldLabel} must be a JSON object.`;
      return null;
    }
    return parsed as Record<string, unknown>;
  }

  function isNoModelLoadedError(message: string): boolean {
    const lowered = message.toLowerCase();
    return lowered.includes('no model loaded') || lowered.includes('/engine/load first');
  }

  function backendLanePreference(): 'cpu' | 'directml' | 'auto' {
    const backend = (backendStatus?.active_backend ?? '').toLowerCase();
    if (backend === 'cpu') {
      return 'cpu';
    }
    if (backend === 'directml') {
      return 'directml';
    }
    return 'auto';
  }

  function isModelReadyForLane(
    modelReadiness: CheckModelResponse,
    lane: 'cpu' | 'directml' | 'auto'
  ): boolean {
    if (lane === 'cpu') {
      return modelReadiness.lanes.cpu.ready;
    }
    if (lane === 'directml') {
      return modelReadiness.lanes.directml.ready;
    }
    return (
      modelReadiness.lanes.directml.ready ||
      modelReadiness.lanes.cpu.ready ||
      modelReadiness.lanes.openvino_npu.ready
    );
  }

  function uniqueModelCandidates(candidates: string[]): string[] {
    const seen = new Set<string>();
    const unique: string[] = [];
    for (const candidate of candidates) {
      if (!candidate || seen.has(candidate)) {
        continue;
      }
      seen.add(candidate);
      unique.push(candidate);
    }
    return unique;
  }

  async function resolveLaneCompatibleModel(preferredModelId?: string | null): Promise<string | null> {
    if (models.length === 0) {
      await refreshModels();
    }
    if (models.length === 0) {
      return null;
    }

    const lane = backendLanePreference();
    const availableModelIds = models.map((model) => model.id);
    const preferred = preferredModelId?.trim() ? preferredModelId.trim() : selectedModelId;
    const baseCandidates =
      lane === 'cpu'
        ? [preferred, CPU_SAFE_MODEL_ID, ...availableModelIds]
        : [preferred, ...availableModelIds];
    const orderedCandidates = uniqueModelCandidates(
      baseCandidates.filter((candidate): candidate is string => Boolean(candidate && candidate.trim()))
    );

    for (const modelId of orderedCandidates) {
      try {
        const modelReadiness = await invoke<CheckModelResponse>('check_model_readiness', { modelId });
        if (isModelReadyForLane(modelReadiness, lane)) {
          return modelId;
        }
      } catch {
        continue;
      }
    }

    return availableModelIds[0] ?? null;
  }

  async function loadModelById(modelId: string, contextLabel: string): Promise<void> {
    selectedModelId = modelId;
    actionMessage = await withTimeout(
      invoke<string>('load_model', { modelId }),
      MODEL_LOAD_TIMEOUT_MS,
      `Model load timed out after ${MODEL_LOAD_TIMEOUT_MS / 1000}s while ${contextLabel}.`
    );
    await Promise.all([
      refreshBootstrapStatus(),
      refreshCurrentModel(),
      refreshBackendStatus(),
      refreshReadiness()
    ]);
  }

  async function runWithModelReloadRetry<T>(
    operation: () => Promise<T>,
    contextLabel: string,
    traceLabel?: string
  ): Promise<T> {
    try {
      return await operation();
    } catch (error) {
      const message = normalizeEngineError(formatError(error));
      if (!isNoModelLoadedError(message)) {
        throw error;
      }

      const fallbackModelId = await resolveLaneCompatibleModel(selectedModelId);
      if (!fallbackModelId) {
        throw new Error(`${message} No lane-compatible model is available to reload.`);
      }

      await loadModelById(fallbackModelId, contextLabel);
      if (traceLabel) {
        pushWorkflowTrace(
          `${traceLabel}: detected unloaded model state; auto-reloaded '${fallbackModelId}' and retrying.`
        );
      } else {
        actionMessage = `Model '${fallbackModelId}' was auto-reloaded while ${contextLabel}.`;
      }
      return await operation();
    }
  }

  function setToolArgumentTemplate(toolName: string): void {
    const tool = mcpTools.find((candidate) => candidate.name === toolName);
    mcpArguments = JSON.stringify(buildMcpArgsTemplate(tool), null, 2);
  }

  function handleMcpToolSelection(toolName: string): void {
    selectedMcpTool = toolName;
  }

  function handleSelectedModelChange(nextValue: string): void {
    selectedModelId = nextValue;
  }

  function handlePromptChange(nextValue: string): void {
    prompt = nextValue;
  }

  function handleMcpArgumentsChange(nextValue: string): void {
    mcpArguments = nextValue;
  }

  function handleWorkflowPromptChange(nextValue: string): void {
    workflowPrompt = nextValue;
  }

  function handleIssueRequestPayloadChange(nextValue: string): void {
    issueRequestPayload = nextValue;
  }

  function handleIssueHttpStatusChange(nextValue: string): void {
    issueHttpStatus = nextValue;
  }

  function handleIssueResponseBodyChange(nextValue: string): void {
    issueResponseBody = nextValue;
  }

  function setWorkflowSystemPrompt(nextValue: string): void {
    workflowSystemPrompt = nextValue;
  }

  function setWorkflowTemperature(nextValue: number): void {
    if (!Number.isFinite(nextValue)) {
      workflowTemperature = DEFAULT_WORKFLOW_TEMPERATURE;
      return;
    }
    workflowTemperature = Math.max(0, Math.min(2, nextValue));
  }

  function setWorkflowMaxTokens(nextValue: number): void {
    if (!Number.isFinite(nextValue)) {
      workflowMaxTokens = DEFAULT_WORKFLOW_MAX_TOKENS;
      return;
    }
    workflowMaxTokens = Math.max(16, Math.min(8192, Math.floor(nextValue)));
  }

  function setMcpPythonPath(nextValue: string): void {
    const trimmed = nextValue.trim();
    mcpPythonPath = trimmed || DEFAULT_PYTHON_COMMAND;
  }

  async function invokeStartMcpServer(): Promise<McpStatus> {
    const trimmedPythonPath = mcpPythonPath.trim();
    if (trimmedPythonPath) {
      return await invoke<McpStatus>('start_mcp_server', { pythonPath: trimmedPythonPath });
    }
    return await invoke<McpStatus>('start_mcp_server');
  }

  function pushWorkflowTrace(line: string): void {
    workflowTrace = [...workflowTrace, line];
  }

  function resetWorkflowRunState(): void {
    workflowFinalResponse = '';
    workflowTrace = [];
    workflowDepthUsed = 0;
    workflowToolCallsUsed = 0;
    workflowOutcome = 'none';
    workflowErrorDetail = null;
    workflowLastEvidenceFile = null;
    mcpToolResult = null;
  }

  function setWorkflowOutcome(tag: WorkflowOutcomeTag, detail: string): void {
    workflowOutcome = tag;
    if (tag === 'failed_with_error') {
      workflowErrorDetail = detail;
    }
    pushWorkflowTrace(`[outcome:${tag}] ${detail}`);
  }

  function hasWorkflowEvidence(): boolean {
    return (
      workflowTrace.length > 0 ||
      workflowFinalResponse.trim().length > 0 ||
      workflowErrorDetail !== null
    );
  }

  function buildWorkflowEvidencePayload(): Record<string, unknown> {
    return {
      generated_at_iso: new Date().toISOString(),
      workflow_prompt: workflowPrompt.trim(),
      workflow_outcome: workflowOutcome,
      workflow_depth_used: workflowDepthUsed,
      workflow_tool_calls_used: workflowToolCallsUsed,
      workflow_trace: workflowTrace,
      workflow_final_response: workflowFinalResponse,
      workflow_error: workflowErrorDetail,
      selected_model_id: selectedModelId,
      current_model_id: currentModelId,
      backend_status: backendStatus,
      readiness_snapshot: readiness,
      mcp_status: mcpStatus,
      selected_mcp_tool: selectedMcpTool || null,
      mcp_tool_result: mcpToolResult,
      generation_metrics: lastMetrics
    };
  }

  async function copyWorkflowEvidence(): Promise<void> {
    if (!hasWorkflowEvidence()) {
      commandError = 'Run a workflow first before copying workflow evidence.';
      return;
    }
    clearFeedback();
    try {
      const payload = buildWorkflowEvidencePayload();
      await navigator.clipboard.writeText(JSON.stringify(payload, null, 2));
      actionMessage = 'Workflow evidence JSON copied to clipboard.';
    } catch (error) {
      commandError = `Clipboard copy failed: ${formatError(error)}`;
    }
  }

  function exportWorkflowEvidence(): void {
    if (!hasWorkflowEvidence()) {
      commandError = 'Run a workflow first before exporting workflow evidence.';
      return;
    }
    clearFeedback();
    const payload = buildWorkflowEvidencePayload();
    const serialized = JSON.stringify(payload, null, 2);
    const timestamp = new Date().toISOString().replace(/[.:]/g, '-');
    const fileName = `libreoffice-workflow-evidence-${timestamp}.json`;
    const blob = new Blob([serialized], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = fileName;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
    workflowLastEvidenceFile = fileName;
    actionMessage = 'Workflow evidence exported.';
  }

  async function restartMcpServerWithOptionalTrace(traceLabel?: string): Promise<void> {
    if (traceLabel) {
      pushWorkflowTrace(`${traceLabel}: restarting MCP server...`);
    }
    await invoke<McpStatus>('stop_mcp_server');
    mcpStatus = await invokeStartMcpServer();
    if (!mcpStatus.running) {
      throw new Error(mcpStatus.error_message ?? 'MCP restart failed.');
    }
    await loadMcpTools();
    if (traceLabel) {
      pushWorkflowTrace(`${traceLabel}: MCP restart completed.`);
    }
  }

  async function callMcpToolWithRecovery(
    name: string,
    args: unknown,
    traceLabel?: string
  ): Promise<ToolResult> {
    let result = await invoke<ToolResult>('call_mcp_tool', {
      name,
      arguments: args
    });

    if (hasHelperConnectionError(result)) {
      if (traceLabel) {
        pushWorkflowTrace(
          `${traceLabel}: helper connection failed, retrying once after MCP restart.`
        );
      }
      await restartMcpServerWithOptionalTrace(traceLabel);
      result = await invoke<ToolResult>('call_mcp_tool', {
        name,
        arguments: args
      });
    }

    return result;
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
      await Promise.all([refreshBackendStatus(), refreshModels(), refreshCurrentModel()]);
      const startupModelId = await resolveLaneCompatibleModel(selectedModelId);
      if (startupModelId) {
        selectedModelId = startupModelId;
        await refreshReadiness();
      }
      actionMessage = startupModelId
        ? `Engine ensure-started completed. Suggested model for this lane: '${startupModelId}'.`
        : 'Engine ensure-started completed.';
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
    actionMessage = `Loading model '${selectedModelId}'...`;
    try {
      await loadModelById(selectedModelId, 'loading selected model');
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

  async function generateUsingStreamingFallback(promptText: string): Promise<GenerationResult> {
    streamingText = '';
    const onTokenChannel = new Channel<string>();
    onTokenChannel.onmessage = (token) => {
      streamingText += token;
    };

    const metrics = await invoke<GenerationMetrics>('inference_generate', {
      prompt: promptText,
      onToken: onTokenChannel
    });
    return {
      text: streamingText,
      metrics
    };
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
      let result = await runWithModelReloadRetry(
        () => invoke<GenerationResult>('generate_text', { prompt }),
        'running non-stream generation'
      );

      if (!result.text.trim()) {
        result = await runWithModelReloadRetry(
          () => generateUsingStreamingFallback(prompt),
          'running non-stream fallback generation'
        );
        if (result.text.trim()) {
          actionMessage = 'Non-stream response was empty; recovered output via streaming fallback.';
        } else {
          commandError =
            'Generation completed but returned empty output. Try a different prompt or model.';
        }
      }

      generatedText = result.text;
      lastMetrics = result.metrics;
      if (!actionMessage && !commandError) {
        actionMessage = 'Non-stream generation completed.';
      }
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
      const metrics = await runWithModelReloadRetry(
        () =>
          invoke<GenerationMetrics>('inference_generate', {
            prompt,
            onToken: onTokenChannel
          }),
        'running streaming generation'
      );
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
      ? parseJsonInput(issueRequestPayload, 'Issue request payload')
      : null;
    if (issueRequestPayload.trim() && requestPayload === null) {
      return null;
    }
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

  async function refreshMcpStatus(): Promise<void> {
    try {
      mcpStatus = await invoke<McpStatus>('check_mcp_status');
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    }
  }

  async function startMcpServer(): Promise<void> {
    actionBusy = true;
    clearFeedback();
    try {
      mcpStatus = await invokeStartMcpServer();
      if (mcpStatus.running) {
        await loadMcpTools();
        actionMessage = `MCP server started (${mcpTools.length} tools loaded).`;
      } else {
        actionMessage = 'MCP server failed to start.';
      }
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function stopMcpServer(): Promise<void> {
    actionBusy = true;
    clearFeedback();
    try {
      mcpStatus = await invoke<McpStatus>('stop_mcp_server');
      mcpTools = [];
      selectedMcpTool = '';
      mcpToolResult = null;
      actionMessage = 'MCP server stop requested.';
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function loadMcpTools(): Promise<void> {
    try {
      const previousSelected = selectedMcpTool;
      mcpTools = await invoke<McpTool[]>('list_mcp_tools');
      if (!mcpTools.some((tool) => tool.name === selectedMcpTool) && mcpTools.length > 0) {
        selectedMcpTool = mcpTools[0].name;
      }
      if (
        selectedMcpTool &&
        (selectedMcpTool !== previousSelected || !mcpArguments.trim() || mcpArguments.trim() === '{}')
      ) {
        setToolArgumentTemplate(selectedMcpTool);
      }
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    }
  }

  async function callSelectedMcpTool(): Promise<void> {
    if (!selectedMcpTool.trim()) {
      commandError = 'Select an MCP tool before invoking.';
      return;
    }

    actionBusy = true;
    clearFeedback();
    try {
      const parsedArgs = mcpArguments.trim()
        ? parseJsonObjectInput(mcpArguments, 'MCP arguments')
        : {};
      if (parsedArgs === null) {
        return;
      }
      mcpToolResult = await callMcpToolWithRecovery(selectedMcpTool, parsedArgs);
      actionMessage = `MCP tool '${selectedMcpTool}' executed.`;
    } catch (error) {
      commandError = normalizeEngineError(formatError(error));
    } finally {
      actionBusy = false;
    }
  }

  async function ensureMcpReadyForWorkflow(): Promise<void> {
    mcpStatus = await invoke<McpStatus>('check_mcp_status');
    if (!mcpStatus.running) {
      pushWorkflowTrace('MCP server is not running. Starting it now...');
      mcpStatus = await invokeStartMcpServer();
    }
    if (!mcpStatus.running) {
      throw new Error(mcpStatus.error_message ?? 'MCP server failed to start.');
    }
    if (mcpTools.length === 0) {
      await loadMcpTools();
    }
    if (mcpTools.length === 0) {
      throw new Error('MCP preflight failed: no tools were loaded.');
    }
  }

  async function ensureWorkflowPreflight(): Promise<'cpu' | 'directml'> {
    bootstrap = await invoke<BootstrapStatus>('ensure_engine_started');
    if (!bootstrap.engine.healthy) {
      throw new Error('Workflow preflight failed: engine is not healthy.');
    }

    await Promise.all([refreshModels(), refreshCurrentModel(), refreshBackendStatus()]);
    const lane = backendLanePreference();
    if (lane === 'auto') {
      throw new Error(
        "Workflow preflight requires a known backend lane (cpu or directml). Refresh backend status and retry."
      );
    }
    if (!selectedModelId.trim()) {
      throw new Error('Workflow preflight requires a selected model.');
    }

    const selectedReadiness = await invoke<CheckModelResponse>('check_model_readiness', {
      modelId: selectedModelId
    });
    readiness = selectedReadiness;
    if (!isModelReadyForLane(selectedReadiness, lane)) {
      const laneCompatibleModel = await resolveLaneCompatibleModel(selectedModelId);
      if (!laneCompatibleModel) {
        throw new Error(`Workflow preflight failed: no model is ready for lane '${lane}'.`);
      }
      pushWorkflowTrace(
        `Preflight: '${selectedModelId}' is not ready for lane '${lane}', switching to '${laneCompatibleModel}'.`
      );
      await loadModelById(laneCompatibleModel, `workflow preflight (${lane} lane)`);
    } else if (currentModelId !== selectedModelId) {
      pushWorkflowTrace(`Preflight: loading selected model '${selectedModelId}' for workflow run.`);
      await loadModelById(selectedModelId, `workflow preflight (${lane} lane)`);
    }

    const laneReadiness = await invoke<CheckModelResponse>('check_model_readiness', {
      modelId: selectedModelId
    });
    readiness = laneReadiness;
    if (!isModelReadyForLane(laneReadiness, lane)) {
      throw new Error(
        `Workflow preflight failed: model '${selectedModelId}' is not ready for lane '${lane}' (reason: ${laneReadiness.lanes[lane].reason}).`
      );
    }

    await ensureMcpReadyForWorkflow();
    pushWorkflowTrace(
      `Preflight complete: lane=${lane}, model=${selectedModelId}, mcp_tools=${mcpTools.length}.`
    );
    return lane;
  }

  async function runMcpAssistedWorkflow(): Promise<void> {
    if (!workflowPrompt.trim()) {
      commandError = 'Enter a workflow prompt before running the Phase 3 flow.';
      return;
    }

    actionBusy = true;
    clearFeedback();
    resetWorkflowRunState();

    try {
      const lane = await ensureWorkflowPreflight();
      pushWorkflowTrace(`Loaded ${mcpTools.length} MCP tools.`);

      const turns: ChatTurn[] = [
        {
          role: 'system',
          content: buildPhase3SystemInstruction()
        },
        {
          role: 'user',
          content: workflowPrompt.trim()
        }
      ];

      let finalResponse: string | null = null;
      let totalToolCalls = 0;

      for (let depth = 1; depth <= PHASE3_MAX_TOOL_CHAIN_DEPTH; depth += 1) {
        workflowDepthUsed = depth;
        pushWorkflowTrace(`Turn ${depth}: requesting model response...`);
        const modelResult = await runWithModelReloadRetry(
          () =>
            withTimeout(
              invoke<GenerationResult>('generate_text_with_config', {
                prompt: buildChatMlPrompt(turns),
                config: buildWorkflowGenerationConfig()
              }),
              PHASE3_MODEL_TURN_TIMEOUT_MS,
              `Turn ${depth} timed out after ${PHASE3_MODEL_TURN_TIMEOUT_MS / 1000}s.`,
              cancelGenerationAfterTimeout
            ),
          `running workflow model turn ${depth}`,
          `Turn ${depth}`
        );
        lastMetrics = modelResult.metrics;
        const assistantText = modelResult.text.trim();
        turns.push({
          role: 'assistant',
          content: assistantText
        });
        pushWorkflowTrace(`Turn ${depth}: model response received (${assistantText.length} chars).`);

        const toolCalls = extractToolCallsFromModelText(
          assistantText,
          PHASE3_MAX_TOOL_CALLS_PER_RESPONSE
        );
        if (toolCalls.length === 0) {
          finalResponse = assistantText;
          break;
        }

        for (const toolCall of toolCalls) {
          totalToolCalls += 1;
          workflowToolCallsUsed = totalToolCalls;
          pushWorkflowTrace(
            `Tool call ${totalToolCalls}: ${toolCall.name} ${JSON.stringify(toolCall.arguments)}`
          );
          const toolResult = await callMcpToolWithRecovery(
            toolCall.name,
            toolCall.arguments,
            `Tool call ${totalToolCalls}`
          );
          mcpToolResult = toolResult;
          pushWorkflowTrace(`Tool result ${totalToolCalls}: ${summarizeToolResult(toolResult)}`);
          if (hasToolExecutionError(toolResult)) {
            throw new Error(`Tool call ${totalToolCalls} failed: ${summarizeToolResult(toolResult)}`);
          }
          turns.push({
            role: 'system',
            content:
              `MCP tool result for '${toolCall.name}':\n` +
              `${JSON.stringify(toolResult)}\n` +
              'Use this result to continue. Return another tool_call JSON only if needed.'
          });
        }
      }

      if (!finalResponse) {
        const depthError =
          'Stopped before final answer because the workflow reached the maximum tool-chain depth.';
        workflowFinalResponse = depthError;
        generatedText = depthError;
        setWorkflowOutcome('failed_with_error', depthError);
        actionMessage = 'Phase 3 MCP-assisted workflow stopped before final answer.';
        return;
      }

      workflowFinalResponse = finalResponse;
      generatedText = finalResponse;
      setWorkflowOutcome(
        'model_assisted_success',
        `Completed on lane '${lane}' with ${totalToolCalls} tool call(s).`
      );
      actionMessage = 'Phase 3 MCP-assisted workflow completed.';
    } catch (error) {
      const message = normalizeEngineError(formatError(error));
      setWorkflowOutcome('failed_with_error', message);
      commandError = message;
    } finally {
      actionBusy = false;
      await Promise.all([
        refreshBootstrapStatus(),
        refreshBackendStatus(),
        refreshCurrentModel(),
        refreshMcpStatus()
      ]);
    }
  }

  async function runToolFirstWorkflow(): Promise<void> {
    const toolName = selectedMcpTool.trim();
    if (!toolName) {
      commandError = 'Select an MCP tool in the Source-Parity Tools tab first.';
      return;
    }

    actionBusy = true;
    clearFeedback();
    resetWorkflowRunState();
    let lane: 'cpu' | 'directml' | null = null;

    try {
      lane = await ensureWorkflowPreflight();
      pushWorkflowTrace(`Fast path: invoking '${toolName}' directly.`);

      const parsedArgs = mcpArguments.trim()
        ? parseJsonObjectInput(mcpArguments, 'MCP arguments')
        : {};
      if (parsedArgs === null) {
        return;
      }
      const toolResult = await callMcpToolWithRecovery(
        toolName,
        parsedArgs,
        `Fast path '${toolName}'`
      );
      mcpToolResult = toolResult;
      if (hasToolExecutionError(toolResult)) {
        throw new Error(`Fast path tool call failed: ${summarizeToolResult(toolResult)}`);
      }
      workflowToolCallsUsed = 1;
      workflowDepthUsed = 1;
      pushWorkflowTrace(`Tool call succeeded: ${toolName}`);

      if (lane === 'cpu') {
        workflowFinalResponse = buildLocalFallbackSummary(toolResult);
        setWorkflowOutcome(
          'cpu_local_fallback',
          'CPU lane detected; skipped summary model turn and used deterministic local summary.'
        );
        actionMessage = 'Phase 3 fast workflow completed (CPU local summary).';
        return;
      }

      const summaryPrompt = buildChatMlPrompt([
        {
          role: 'system',
          content:
            'Summarize the provided MCP tool result for the user in 2 short sentences. Plain text only.'
        },
        {
          role: 'user',
          content:
            `Original user request:\n${workflowPrompt.trim()}\n\n` +
            `Tool result JSON:\n${JSON.stringify(toolResult)}`
        }
      ]);

      pushWorkflowTrace('Fast path: requesting short summary turn...');
      const summary = await runWithModelReloadRetry(
        () =>
          withTimeout(
            invoke<GenerationResult>('generate_text_with_config', {
              prompt: summaryPrompt,
              config: buildWorkflowGenerationConfig()
            }),
            30000,
            'Summary turn timed out after 30s.',
            cancelGenerationAfterTimeout
          ),
        'running fast-path summary generation',
        'Fast path'
      );
      workflowDepthUsed = 2;
      lastMetrics = summary.metrics;
      workflowFinalResponse = summary.text.trim();
      pushWorkflowTrace('Summary turn completed.');
      setWorkflowOutcome(
        'model_assisted_success',
        "Tool-first flow completed with model summary turn."
      );
      actionMessage = 'Phase 3 fast workflow completed.';
    } catch (error) {
      const message = normalizeEngineError(formatError(error));
      if (message.includes('timed out') && lane === 'cpu') {
        const fallbackSummary = mcpToolResult
          ? buildLocalFallbackSummary(mcpToolResult)
          : 'Tool call succeeded, but summary generation timed out on this machine.';
        workflowFinalResponse = fallbackSummary;
        setWorkflowOutcome(
          'cpu_local_fallback',
          'Summary turn timed out on CPU lane; used deterministic local fallback summary.'
        );
        actionMessage = 'Phase 3 fast workflow completed with summary timeout on CPU.';
      } else {
        setWorkflowOutcome('failed_with_error', message);
        commandError = message;
      }
    } finally {
      actionBusy = false;
      await Promise.all([
        refreshBootstrapStatus(),
        refreshBackendStatus(),
        refreshCurrentModel(),
        refreshMcpStatus()
      ]);
    }
  }

export const libreofficeController = {
  get loadingBootstrap() {
    return loadingBootstrap;
  },
  get actionBusy() {
    return actionBusy;
  },
  get commandError() {
    return commandError;
  },
  get actionMessage() {
    return actionMessage;
  },
  get bootstrap() {
    return bootstrap;
  },
  get models() {
    return models;
  },
  get selectedModelId() {
    return selectedModelId;
  },
  get currentModelId() {
    return currentModelId;
  },
  get readiness() {
    return readiness;
  },
  get backendStatus() {
    return backendStatus;
  },
  get prompt() {
    return prompt;
  },
  get generatedText() {
    return generatedText;
  },
  get streamingText() {
    return streamingText;
  },
  get lastMetrics() {
    return lastMetrics;
  },
  get streaming() {
    return streaming;
  },
  get issueRequestPayload() {
    return issueRequestPayload;
  },
  get issueHttpStatus() {
    return issueHttpStatus;
  },
  get issueResponseBody() {
    return issueResponseBody;
  },
  get integrationIssueReport() {
    return integrationIssueReport;
  },
  get runtimeVerification() {
    return runtimeVerification;
  },
  get evidenceExportPath() {
    return evidenceExportPath;
  },
  get mcpStatus() {
    return mcpStatus;
  },
  get mcpTools() {
    return mcpTools;
  },
  get selectedMcpTool() {
    return selectedMcpTool;
  },
  get mcpArguments() {
    return mcpArguments;
  },
  get mcpToolResult() {
    return mcpToolResult;
  },
  get workflowPrompt() {
    return workflowPrompt;
  },
  get workflowFinalResponse() {
    return workflowFinalResponse;
  },
  get workflowTrace() {
    return workflowTrace;
  },
  get workflowDepthUsed() {
    return workflowDepthUsed;
  },
  get workflowToolCallsUsed() {
    return workflowToolCallsUsed;
  },
  get workflowOutcome() {
    return workflowOutcome;
  },
  get workflowErrorDetail() {
    return workflowErrorDetail;
  },
  get workflowLastEvidenceFile() {
    return workflowLastEvidenceFile;
  },
  get workflowSystemPrompt() {
    return workflowSystemPrompt;
  },
  get workflowTemperature() {
    return workflowTemperature;
  },
  get workflowMaxTokens() {
    return workflowMaxTokens;
  },
  get mcpPythonPath() {
    return mcpPythonPath;
  },

  setSelectedModelId(nextValue: string): void {
    selectedModelId = nextValue;
  },

  setPrompt(nextValue: string): void {
    prompt = nextValue;
  },

  setMcpToolSelection: handleMcpToolSelection,
  setMcpArguments: handleMcpArgumentsChange,
  setWorkflowPrompt: handleWorkflowPromptChange,
  setWorkflowSystemPrompt,
  setWorkflowTemperature,
  setWorkflowMaxTokens,
  setMcpPythonPath,
  setIssueRequestPayload: handleIssueRequestPayloadChange,
  setIssueHttpStatus: handleIssueHttpStatusChange,
  setIssueResponseBody: handleIssueResponseBodyChange,
  setToolArgumentTemplate,

  hasWorkflowEvidence,
  refreshBootstrapStatus,
  ensureEngineStarted,
  refreshReadiness,
  refreshModels,
  loadSelectedModel,
  unloadCurrentModel,
  generateNonStream,
  generateStream,
  cancelGeneration,
  refreshMcpStatus,
  startMcpServer,
  stopMcpServer,
  loadMcpTools,
  callSelectedMcpTool,
  runMcpAssistedWorkflow,
  runToolFirstWorkflow,
  copyWorkflowEvidence,
  exportWorkflowEvidence,
  runRuntimeChecklist,
  createIssueReport,
  exportEvidenceBundle,
  copyIssueReport,

  async initialize(): Promise<void> {
    clearFeedback();
    await refreshBootstrapStatus();
    await refreshModels();
    await Promise.all([refreshCurrentModel(), refreshBackendStatus()]);
    const startupModelId = await resolveLaneCompatibleModel(currentModelId ?? selectedModelId);
    if (startupModelId) {
      selectedModelId = startupModelId;
    }
    await refreshReadiness();
    await refreshMcpStatus();
    if (mcpStatus?.running) {
      await loadMcpTools();
    }
  }
};
