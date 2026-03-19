export type BootstrapEngineSnapshot = {
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

export type BootstrapStatus = {
  stage: string;
  notes: string[];
  engine: BootstrapEngineSnapshot;
};

export type ModelDefinition = {
  id: string;
  name: string;
  size: string;
  disk_size_gb: number;
  min_ram_gb: number;
  directory: string;
  description: string;
};

export type GenerationMetrics = {
  total_tokens: number;
  time_to_first_token_ms: number | null;
  tokens_per_second: number;
  total_time_ms: number;
};

export type GenerationResult = {
  text: string;
  metrics: GenerationMetrics;
};

export type GenerationConfig = {
  max_length: number;
  temperature: number;
  top_k?: number | null;
  top_p?: number | null;
  repetition_penalty?: number | null;
  repetition_penalty_last_n?: number | null;
};

export type LaneReadiness = {
  artifact_ready: boolean;
  bundle_ready: boolean;
  ready: boolean;
  reason: string;
};

export type CheckModelResponse = {
  model_id: string;
  lanes: {
    openvino_npu: LaneReadiness;
    directml: LaneReadiness;
    cpu: LaneReadiness;
  };
};

export type BackendStatus = {
  active_backend?: string | null;
  runtime_engine?: string | null;
  selection_state?: string | null;
  selection_reason?: string | null;
  available_backends?: string[];
};

export type IntegrationIssueReport = {
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

export type VerificationCheck = {
  id: string;
  ok: boolean;
  detail: string;
};

export type RuntimeVerificationReport = {
  generated_at_unix: number;
  model_id: string;
  checks: VerificationCheck[];
  all_passed: boolean;
};

export type EvidenceExportResult = {
  path: string;
  runtime_verification: RuntimeVerificationReport;
  integration_issue_report: IntegrationIssueReport;
};

export type McpStatus = {
  running: boolean;
  error_message?: string | null;
};

export type McpTool = {
  name: string;
  description: string;
  input_schema?: unknown;
  output_schema?: unknown;
};

export type JsonSchema = {
  type?: string;
  anyOf?: JsonSchema[];
  default?: unknown;
  properties?: Record<string, JsonSchema>;
  required?: string[];
  items?: JsonSchema;
};

export type ToolContent = {
  type: string;
  text: string;
};

export type ToolResult = {
  content: ToolContent[];
  is_error?: boolean;
};

export type ChatRole = 'system' | 'user' | 'assistant';

export type ChatTurn = {
  role: ChatRole;
  content: string;
};

export type WorkflowToolCall = {
  name: string;
  arguments: Record<string, unknown>;
};

export type WorkflowOutcomeTag =
  | 'none'
  | 'model_assisted_success'
  | 'cpu_local_fallback'
  | 'failed_with_error';
