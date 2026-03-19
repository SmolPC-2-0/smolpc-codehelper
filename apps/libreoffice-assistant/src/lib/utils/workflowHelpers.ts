import type {
  ChatTurn,
  JsonSchema,
  McpTool,
  ToolResult,
  WorkflowToolCall
} from '../types/libreoffice';

function chooseNonNullSchema(schema: JsonSchema | undefined): JsonSchema | undefined {
  if (!schema) {
    return undefined;
  }
  if (schema.anyOf && schema.anyOf.length > 0) {
    return schema.anyOf.find((candidate) => candidate.type !== 'null') ?? schema.anyOf[0];
  }
  return schema;
}

function schemaTemplateValue(schema: JsonSchema | undefined): unknown {
  const resolved = chooseNonNullSchema(schema);
  if (!resolved) {
    return '';
  }
  if (resolved.default !== undefined) {
    return resolved.default;
  }
  if (resolved.type === 'object') {
    return {};
  }
  if (resolved.type === 'array') {
    return [];
  }
  if (resolved.type === 'boolean') {
    return false;
  }
  if (resolved.type === 'integer' || resolved.type === 'number') {
    return 0;
  }
  return '';
}

export function buildMcpArgsTemplate(tool: McpTool | undefined): Record<string, unknown> {
  const schema = tool?.input_schema as JsonSchema | undefined;
  const properties = schema?.properties;
  if (!properties) {
    return {};
  }

  const required = new Set(schema?.required ?? []);
  const template: Record<string, unknown> = {};
  for (const [key, propertySchema] of Object.entries(properties)) {
    if (required.has(key) || propertySchema.default !== undefined) {
      template[key] = schemaTemplateValue(propertySchema);
    }
  }
  return template;
}

function schemaTypeLabel(schema: JsonSchema | undefined): string {
  const resolved = chooseNonNullSchema(schema);
  if (!resolved) {
    return 'any';
  }
  if (resolved.type) {
    return resolved.type;
  }
  if (resolved.anyOf && resolved.anyOf.length > 0) {
    const labels = resolved.anyOf
      .map((candidate) => candidate.type)
      .filter((label): label is string => Boolean(label));
    if (labels.length > 0) {
      return labels.join('|');
    }
  }
  return 'any';
}

function buildToolSignature(tool: McpTool): string {
  const schema = tool.input_schema as JsonSchema | undefined;
  const properties = schema?.properties ?? {};
  const required = new Set(schema?.required ?? []);
  const entries = Object.entries(properties);
  if (entries.length === 0) {
    return `${tool.name}()`;
  }

  const ordered = entries.sort((a, b) => {
    const aRequired = required.has(a[0]) ? 0 : 1;
    const bRequired = required.has(b[0]) ? 0 : 1;
    if (aRequired !== bRequired) {
      return aRequired - bRequired;
    }
    return a[0].localeCompare(b[0]);
  });

  const args = ordered
    .slice(0, 6)
    .map(([name, value]) => `${name}${required.has(name) ? '' : '?'}:${schemaTypeLabel(value)}`)
    .join(', ');

  return `${tool.name}(${args})`;
}

export function buildWorkflowToolCatalogInstruction(tools: McpTool[]): string {
  if (!tools.length) {
    return 'No MCP tools are currently available. Do not emit tool_call JSON.';
  }

  const signatures = tools
    .map((tool) => `- ${buildToolSignature(tool)}`)
    .join('\n');

  return (
    'Available MCP tools (use exact tool names only):\n' +
    `${signatures}\n` +
    'If a required argument is unknown, choose a safe default and continue.'
  );
}

export function buildChatMlPrompt(turns: ChatTurn[]): string {
  const lines = turns.map((turn) => `<|im_start|>${turn.role}\n${turn.content}<|im_end|>`);
  return `${lines.join('\n')}\n<|im_start|>assistant\n`;
}

function asObject(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function parseToolArguments(value: unknown): Record<string, unknown> {
  const objectValue = asObject(value);
  if (objectValue) {
    return objectValue;
  }
  if (typeof value === 'string' && value.trim()) {
    try {
      const parsed = JSON.parse(value) as unknown;
      return asObject(parsed) ?? {};
    } catch {
      return {};
    }
  }
  return {};
}

function normalizeKnownFolderPath(value: unknown): string | null {
  if (typeof value !== 'string') {
    return null;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const normalized = trimmed.replace(/\\/g, '/').toLowerCase();
  if (normalized === 'documents' || normalized === 'document' || normalized === 'my documents') {
    return '~/Documents';
  }
  if (normalized === 'desktop') {
    return '~/Desktop';
  }
  if (normalized === 'downloads' || normalized === 'download') {
    return '~/Downloads';
  }

  return trimmed;
}

function normalizeWorkflowToolCall(
  rawName: string,
  rawArguments: Record<string, unknown>
): WorkflowToolCall {
  const nameKey = rawName.trim().toLowerCase();
  const normalizedArgs: Record<string, unknown> = { ...rawArguments };

  let normalizedName = rawName.trim();
  if (nameKey === 'list_files' || nameKey === 'list_docs') {
    normalizedName = 'list_documents';
  } else if (nameKey === 'create_document') {
    normalizedName = 'create_blank_document';
  }

  if (normalizedName === 'list_documents') {
    const directoryAlias = normalizeKnownFolderPath(
      normalizedArgs.directory ??
        normalizedArgs.folder ??
        normalizedArgs.path ??
        normalizedArgs.location
    );
    const directory =
      directoryAlias ??
      (typeof normalizedArgs.directory === 'string' && normalizedArgs.directory.trim()
        ? normalizedArgs.directory.trim()
        : null) ??
      '~/Documents';

    normalizedArgs.directory = directory;
    delete normalizedArgs.folder;
    delete normalizedArgs.path;
    delete normalizedArgs.location;
    delete normalizedArgs.filter;
  }

  if (normalizedName === 'create_blank_document') {
    const rawFileName =
      (typeof normalizedArgs.file_name === 'string' && normalizedArgs.file_name.trim()
        ? normalizedArgs.file_name.trim()
        : null) ??
      (typeof normalizedArgs.filename === 'string' && normalizedArgs.filename.trim()
        ? normalizedArgs.filename.trim()
        : null) ??
      (typeof normalizedArgs.file_path === 'string' && normalizedArgs.file_path.trim()
        ? normalizedArgs.file_path.trim()
        : null) ??
      (typeof normalizedArgs.path === 'string' && normalizedArgs.path.trim()
        ? normalizedArgs.path.trim()
        : null) ??
      'untitled.odt';
    const requestedFormat =
      (typeof normalizedArgs.file_format === 'string' && normalizedArgs.file_format.trim()
        ? normalizedArgs.file_format.trim().replace(/^\./, '').toLowerCase()
        : null) ?? null;

    let filename = rawFileName;
    if (requestedFormat && !rawFileName.toLowerCase().endsWith(`.${requestedFormat}`)) {
      filename = `${rawFileName}.${requestedFormat}`;
    } else if (!requestedFormat && !rawFileName.includes('.')) {
      filename = `${rawFileName}.odt`;
    }

    normalizedArgs.filename = filename;
    delete normalizedArgs.file_name;
    delete normalizedArgs.file_format;
    delete normalizedArgs.destination;
    delete normalizedArgs.directory;
    delete normalizedArgs.file_path;
    delete normalizedArgs.path;
  }

  if (normalizedName === 'create_blank_presentation') {
    const filename =
      (typeof normalizedArgs.filename === 'string' && normalizedArgs.filename.trim()
        ? normalizedArgs.filename.trim()
        : null) ??
      (typeof normalizedArgs.file_path === 'string' && normalizedArgs.file_path.trim()
        ? normalizedArgs.file_path.trim()
        : null) ??
      (typeof normalizedArgs.path === 'string' && normalizedArgs.path.trim()
        ? normalizedArgs.path.trim()
        : null) ??
      'untitled.odp';
    normalizedArgs.filename = filename;
    delete normalizedArgs.file_path;
    delete normalizedArgs.path;
  }

  return {
    name: normalizedName,
    arguments: normalizedArgs
  };
}

function parseSingleToolCall(value: unknown): WorkflowToolCall | null {
  const objectValue = asObject(value);
  if (!objectValue) {
    return null;
  }

  const functionField = asObject(objectValue.function);
  const rawName =
    typeof objectValue.name === 'string'
      ? objectValue.name
      : typeof functionField?.name === 'string'
        ? functionField.name
        : '';
  if (!rawName.trim()) {
    return null;
  }

  const rawArguments =
    objectValue.arguments ?? objectValue.args ?? functionField?.arguments ?? functionField?.args;
  return normalizeWorkflowToolCall(rawName, parseToolArguments(rawArguments));
}

function normalizeToolCalls(value: unknown): WorkflowToolCall[] {
  if (Array.isArray(value)) {
    return value
      .map((item) => parseSingleToolCall(item))
      .filter((item): item is WorkflowToolCall => item !== null);
  }

  const objectValue = asObject(value);
  if (!objectValue) {
    return [];
  }

  if ('tool_calls' in objectValue) {
    return normalizeToolCalls(objectValue.tool_calls);
  }
  if ('tool_call' in objectValue) {
    return normalizeToolCalls(objectValue.tool_call);
  }

  const direct = parseSingleToolCall(objectValue);
  return direct ? [direct] : [];
}

function extractJsonCandidates(rawText: string): string[] {
  const trimmed = rawText.trim();
  const candidates = trimmed ? [trimmed] : [];
  const codeBlockRegex = /```(?:json)?\s*([\s\S]*?)```/gi;
  let match: RegExpExecArray | null;
  while ((match = codeBlockRegex.exec(rawText)) !== null) {
    const candidate = match[1]?.trim();
    if (candidate) {
      candidates.push(candidate);
    }
  }
  return candidates;
}

export function extractToolCallsFromModelText(
  rawText: string,
  maxToolCallsPerResponse: number
): WorkflowToolCall[] {
  for (const candidate of extractJsonCandidates(rawText)) {
    try {
      const parsed = JSON.parse(candidate) as unknown;
      const toolCalls = normalizeToolCalls(parsed);
      if (toolCalls.length > 0) {
        return toolCalls.slice(0, maxToolCallsPerResponse);
      }
    } catch {
      continue;
    }
  }
  return [];
}

function extractPrimaryToolText(result: ToolResult): string {
  return result.content.find((entry) => entry.type === 'text')?.text ?? '';
}

export function summarizeToolResult(result: ToolResult): string {
  const text = result.content.find((entry) => entry.type === 'text')?.text ?? JSON.stringify(result);
  if (text.length <= 220) {
    return text;
  }
  return `${text.slice(0, 220)}...`;
}

export function hasHelperConnectionError(result: ToolResult): boolean {
  const text = extractPrimaryToolText(result).toLowerCase();
  return text.includes('connection refused') || text.includes('helper script running');
}

export function hasToolExecutionError(result: ToolResult): boolean {
  if (result.is_error === true) {
    return true;
  }

  const text = extractPrimaryToolText(result).trim().toLowerCase();
  if (!text) {
    return false;
  }

  return (
    text.startsWith('error:') ||
    text.startsWith('failed to') ||
    text.includes('unauthorized helper request') ||
    text.includes('unknown tool:')
  );
}

export function buildLocalFallbackSummary(result: ToolResult): string {
  const text = extractPrimaryToolText(result);
  if (!text.trim()) {
    return 'MCP tool call succeeded, but no text content was returned.';
  }

  const countMatch = text.match(/Found\s+(\d+)\s+documents?/i);
  const docCount = countMatch ? Number.parseInt(countMatch[1], 10) : null;
  const nameMatches = Array.from(text.matchAll(/Name:\s*(.+)/g))
    .map((match) => match[1]?.trim())
    .filter((name): name is string => Boolean(name))
    .slice(0, 3);

  if (docCount !== null && nameMatches.length > 0) {
    const names = nameMatches.join(', ');
    return `Found ${docCount} document(s) in the target directory. Example files: ${names}.`;
  }
  if (docCount !== null) {
    return `Found ${docCount} document(s) in the target directory.`;
  }
  return text.length <= 280 ? text : `${text.slice(0, 280)}...`;
}
