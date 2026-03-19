<script lang="ts">
  import { onMount } from 'svelte';
  import BackendStatusPanel from './lib/components/BackendStatusPanel.svelte';
  import BootstrapControls from './lib/components/BootstrapControls.svelte';
  import BootstrapPanel from './lib/components/BootstrapPanel.svelte';
  import GenerationPanel from './lib/components/GenerationPanel.svelte';
  import IssueReportPanel from './lib/components/IssueReportPanel.svelte';
  import McpBridgePanel from './lib/components/McpBridgePanel.svelte';
  import ModelControlPanel from './lib/components/ModelControlPanel.svelte';
  import ReadinessPanel from './lib/components/ReadinessPanel.svelte';
  import RuntimeVerificationPanel from './lib/components/RuntimeVerificationPanel.svelte';
  import WorkflowPanel from './lib/components/WorkflowPanel.svelte';
  import { libreofficeController } from './lib/stores/libreofficeController.svelte';

  const loadingBootstrap = $derived(libreofficeController.loadingBootstrap);
  const actionBusy = $derived(libreofficeController.actionBusy);
  const commandError = $derived(libreofficeController.commandError);
  const actionMessage = $derived(libreofficeController.actionMessage);
  const bootstrap = $derived(libreofficeController.bootstrap);
  const models = $derived(libreofficeController.models);
  const selectedModelId = $derived(libreofficeController.selectedModelId);
  const currentModelId = $derived(libreofficeController.currentModelId);
  const readiness = $derived(libreofficeController.readiness);
  const backendStatus = $derived(libreofficeController.backendStatus);
  const prompt = $derived(libreofficeController.prompt);
  const generatedText = $derived(libreofficeController.generatedText);
  const streamingText = $derived(libreofficeController.streamingText);
  const lastMetrics = $derived(libreofficeController.lastMetrics);
  const streaming = $derived(libreofficeController.streaming);
  const issueRequestPayload = $derived(libreofficeController.issueRequestPayload);
  const issueHttpStatus = $derived(libreofficeController.issueHttpStatus);
  const issueResponseBody = $derived(libreofficeController.issueResponseBody);
  const integrationIssueReport = $derived(libreofficeController.integrationIssueReport);
  const runtimeVerification = $derived(libreofficeController.runtimeVerification);
  const evidenceExportPath = $derived(libreofficeController.evidenceExportPath);
  const mcpStatus = $derived(libreofficeController.mcpStatus);
  const mcpTools = $derived(libreofficeController.mcpTools);
  const selectedMcpTool = $derived(libreofficeController.selectedMcpTool);
  const mcpArguments = $derived(libreofficeController.mcpArguments);
  const mcpToolResult = $derived(libreofficeController.mcpToolResult);
  const workflowPrompt = $derived(libreofficeController.workflowPrompt);
  const workflowFinalResponse = $derived(libreofficeController.workflowFinalResponse);
  const workflowTrace = $derived(libreofficeController.workflowTrace);
  const workflowDepthUsed = $derived(libreofficeController.workflowDepthUsed);
  const workflowToolCallsUsed = $derived(libreofficeController.workflowToolCallsUsed);
  const workflowOutcome = $derived(libreofficeController.workflowOutcome);
  const workflowErrorDetail = $derived(libreofficeController.workflowErrorDetail);
  const workflowLastEvidenceFile = $derived(libreofficeController.workflowLastEvidenceFile);

  function handleMcpToolSelection(toolName: string): void {
    libreofficeController.setMcpToolSelection(toolName);
  }

  function handleSelectedModelChange(nextValue: string): void {
    libreofficeController.setSelectedModelId(nextValue);
  }

  function handlePromptChange(nextValue: string): void {
    libreofficeController.setPrompt(nextValue);
  }

  function handleMcpArgumentsChange(nextValue: string): void {
    libreofficeController.setMcpArguments(nextValue);
  }

  function handleWorkflowPromptChange(nextValue: string): void {
    libreofficeController.setWorkflowPrompt(nextValue);
  }

  function handleIssueRequestPayloadChange(nextValue: string): void {
    libreofficeController.setIssueRequestPayload(nextValue);
  }

  function handleIssueHttpStatusChange(nextValue: string): void {
    libreofficeController.setIssueHttpStatus(nextValue);
  }

  function handleIssueResponseBodyChange(nextValue: string): void {
    libreofficeController.setIssueResponseBody(nextValue);
  }

  function hasWorkflowEvidence(): boolean {
    return libreofficeController.hasWorkflowEvidence();
  }

  const setToolArgumentTemplate = (toolName: string): void =>
    libreofficeController.setToolArgumentTemplate(toolName);
  const refreshBootstrapStatus = (): Promise<void> => libreofficeController.refreshBootstrapStatus();
  const ensureEngineStarted = (): Promise<void> => libreofficeController.ensureEngineStarted();
  const refreshReadiness = (): Promise<void> => libreofficeController.refreshReadiness();
  const refreshModels = (): Promise<void> => libreofficeController.refreshModels();
  const loadSelectedModel = (): Promise<void> => libreofficeController.loadSelectedModel();
  const unloadCurrentModel = (): Promise<void> => libreofficeController.unloadCurrentModel();
  const generateNonStream = (): Promise<void> => libreofficeController.generateNonStream();
  const generateStream = (): Promise<void> => libreofficeController.generateStream();
  const cancelGeneration = (): Promise<void> => libreofficeController.cancelGeneration();
  const refreshMcpStatus = (): Promise<void> => libreofficeController.refreshMcpStatus();
  const startMcpServer = (): Promise<void> => libreofficeController.startMcpServer();
  const stopMcpServer = (): Promise<void> => libreofficeController.stopMcpServer();
  const loadMcpTools = (): Promise<void> => libreofficeController.loadMcpTools();
  const callSelectedMcpTool = (): Promise<void> => libreofficeController.callSelectedMcpTool();
  const runMcpAssistedWorkflow = (): Promise<void> => libreofficeController.runMcpAssistedWorkflow();
  const runToolFirstWorkflow = (): Promise<void> => libreofficeController.runToolFirstWorkflow();
  const copyWorkflowEvidence = (): Promise<void> => libreofficeController.copyWorkflowEvidence();
  const exportWorkflowEvidence = (): void => libreofficeController.exportWorkflowEvidence();
  const runRuntimeChecklist = (): Promise<void> => libreofficeController.runRuntimeChecklist();
  const createIssueReport = (): Promise<void> => libreofficeController.createIssueReport();
  const exportEvidenceBundle = (): Promise<void> => libreofficeController.exportEvidenceBundle();
  const copyIssueReport = (): Promise<void> => libreofficeController.copyIssueReport();

  onMount(() => {
    void libreofficeController.initialize();
  });
</script>

<main class="container">
  <h1>SmolPC LibreOffice Assistant</h1>
  <p class="subtitle">Production candidate shell with shared-engine and MCP workflow hardening</p>

  <BootstrapControls
    {loadingBootstrap}
    {actionBusy}
    {commandError}
    {actionMessage}
    onRefreshBootstrapStatus={() => void refreshBootstrapStatus()}
    onEnsureEngineStarted={() => void ensureEngineStarted()}
    onRefreshReadiness={() => void refreshReadiness()}
  />

  <BootstrapPanel {bootstrap} />

  <ModelControlPanel
    {actionBusy}
    {models}
    {selectedModelId}
    {currentModelId}
    onSelectedModelIdChange={handleSelectedModelChange}
    onRefreshModels={() => void refreshModels()}
    onLoadSelectedModel={() => void loadSelectedModel()}
    onUnloadCurrentModel={() => void unloadCurrentModel()}
  />

  <GenerationPanel
    {actionBusy}
    {streaming}
    {prompt}
    {generatedText}
    {streamingText}
    {lastMetrics}
    onPromptChange={handlePromptChange}
    onGenerateNonStream={() => void generateNonStream()}
    onGenerateStream={() => void generateStream()}
    onCancelGeneration={() => void cancelGeneration()}
  />

  <ReadinessPanel {readiness} />

  <BackendStatusPanel {backendStatus} />

  <McpBridgePanel
    {actionBusy}
    {mcpStatus}
    {mcpTools}
    {selectedMcpTool}
    {mcpArguments}
    {mcpToolResult}
    onRefreshMcpStatus={() => void refreshMcpStatus()}
    onStartMcpServer={() => void startMcpServer()}
    onStopMcpServer={() => void stopMcpServer()}
    onLoadMcpTools={() => void loadMcpTools()}
    onCallSelectedMcpTool={() => void callSelectedMcpTool()}
    onSelectedMcpToolChange={handleMcpToolSelection}
    onMcpArgumentsChange={handleMcpArgumentsChange}
    onApplyToolArgumentTemplate={setToolArgumentTemplate}
  />

  <WorkflowPanel
    {actionBusy}
    {workflowPrompt}
    {selectedModelId}
    {selectedMcpTool}
    {workflowDepthUsed}
    {workflowToolCallsUsed}
    {workflowOutcome}
    {workflowErrorDetail}
    {workflowFinalResponse}
    {workflowTrace}
    {workflowLastEvidenceFile}
    hasWorkflowEvidence={hasWorkflowEvidence()}
    onWorkflowPromptChange={handleWorkflowPromptChange}
    onRunMcpAssistedWorkflow={() => void runMcpAssistedWorkflow()}
    onRunToolFirstWorkflow={() => void runToolFirstWorkflow()}
    onCopyWorkflowEvidence={() => void copyWorkflowEvidence()}
    onExportWorkflowEvidence={exportWorkflowEvidence}
  />

  <RuntimeVerificationPanel
    {actionBusy}
    {selectedModelId}
    {runtimeVerification}
    onRunRuntimeChecklist={() => void runRuntimeChecklist()}
  />

  <IssueReportPanel
    {actionBusy}
    {selectedModelId}
    {issueRequestPayload}
    {issueHttpStatus}
    {issueResponseBody}
    {integrationIssueReport}
    {evidenceExportPath}
    onIssueRequestPayloadChange={handleIssueRequestPayloadChange}
    onIssueHttpStatusChange={handleIssueHttpStatusChange}
    onIssueResponseBodyChange={handleIssueResponseBodyChange}
    onCreateIssueReport={() => void createIssueReport()}
    onExportEvidenceBundle={() => void exportEvidenceBundle()}
    onCopyIssueReport={() => void copyIssueReport()}
  />
</main>
