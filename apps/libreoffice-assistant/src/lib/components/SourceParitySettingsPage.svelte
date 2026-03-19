<script lang="ts">
  import type { ModelDefinition } from '../types/libreoffice';
  import { libreofficeSettingsStore } from '../stores/libreofficeSettings.svelte';

  interface Props {
    models: ModelDefinition[];
    onClose?: () => void;
  }

  let { models, onClose }: Props = $props();

  let saveError = $state<string | null>(null);

  async function handleSave(): Promise<void> {
    saveError = null;
    try {
      await libreofficeSettingsStore.saveSettings();
      if (onClose) {
        onClose();
      }
    } catch (error) {
      saveError = error instanceof Error ? error.message : String(error);
    }
  }

  function handleReset(): void {
    libreofficeSettingsStore.resetToDefaults();
  }
</script>

<div class="settings-page">
  <section class="settings-section">
    <h3>Engine Configuration</h3>

    <label for="settings-model">Model</label>
    <select
      id="settings-model"
      value={libreofficeSettingsStore.settings.selected_model}
      onchange={(event) =>
        libreofficeSettingsStore.updateSetting('selected_model', event.currentTarget.value)}
    >
      {#if models.length === 0}
        <option value={libreofficeSettingsStore.settings.selected_model}>
          {libreofficeSettingsStore.settings.selected_model}
        </option>
      {:else}
        {#each models as model}
          <option value={model.id}>{model.name} ({model.id})</option>
        {/each}
      {/if}
    </select>

    <label for="settings-temperature">
      Temperature: {libreofficeSettingsStore.settings.temperature.toFixed(1)}
    </label>
    <input
      id="settings-temperature"
      type="range"
      min="0"
      max="2"
      step="0.1"
      value={libreofficeSettingsStore.settings.temperature}
      oninput={(event) =>
        libreofficeSettingsStore.updateSetting(
          'temperature',
          Number.parseFloat(event.currentTarget.value)
        )}
    />

    <label for="settings-max-tokens">Max Tokens</label>
    <input
      id="settings-max-tokens"
      type="number"
      min="16"
      max="8192"
      value={libreofficeSettingsStore.settings.max_tokens}
      oninput={(event) =>
        libreofficeSettingsStore.updateSetting(
          'max_tokens',
          Number.parseInt(event.currentTarget.value, 10) || 16
        )}
    />

    <label for="settings-workflow-mode">Workflow Mode</label>
    <select
      id="settings-workflow-mode"
      value={libreofficeSettingsStore.settings.workflow_mode}
      onchange={(event) =>
        libreofficeSettingsStore.updateSetting(
          'workflow_mode',
          event.currentTarget.value as 'mcp_assisted' | 'tool_first'
        )}
    >
      <option value="mcp_assisted">MCP Assisted (recommended)</option>
      <option value="tool_first">Tool First (requires selected MCP tool)</option>
    </select>
  </section>

  <section class="settings-section">
    <h3>Paths</h3>

    <label for="settings-python-path">Python Path</label>
    <input
      id="settings-python-path"
      type="text"
      value={libreofficeSettingsStore.settings.python_path}
      oninput={(event) =>
        libreofficeSettingsStore.updateSetting('python_path', event.currentTarget.value)}
      placeholder="python"
    />

    <label for="settings-documents-path">Documents Path</label>
    <input
      id="settings-documents-path"
      type="text"
      value={libreofficeSettingsStore.settings.documents_path}
      oninput={(event) =>
        libreofficeSettingsStore.updateSetting('documents_path', event.currentTarget.value)}
      placeholder="~/Documents"
    />

    <label for="settings-libreoffice-path">LibreOffice Path (optional)</label>
    <input
      id="settings-libreoffice-path"
      type="text"
      value={libreofficeSettingsStore.settings.libreoffice_path ?? ''}
      oninput={(event) =>
        libreofficeSettingsStore.updateSetting(
          'libreoffice_path',
          event.currentTarget.value.trim() || null
        )}
      placeholder="Auto-detect"
    />
  </section>

  <section class="settings-section">
    <h3>Appearance & Prompting</h3>

    <label for="settings-theme">Theme</label>
    <select
      id="settings-theme"
      value={libreofficeSettingsStore.settings.theme}
      onchange={(event) =>
        libreofficeSettingsStore.updateSetting(
          'theme',
          event.currentTarget.value as 'dark' | 'light'
        )}
    >
      <option value="dark">Dark</option>
      <option value="light">Light</option>
    </select>

    <label for="settings-system-prompt">System Prompt (optional)</label>
    <textarea
      id="settings-system-prompt"
      rows="4"
      value={libreofficeSettingsStore.settings.system_prompt ?? ''}
      oninput={(event) =>
        libreofficeSettingsStore.updateSetting('system_prompt', event.currentTarget.value)}
      placeholder="Additional guidance for the assistant."
    ></textarea>
  </section>

  <div class="footer">
    <button type="button" class="secondary" onclick={handleReset}>Reset Defaults</button>
    <div class="actions">
      {#if onClose}
        <button type="button" class="secondary" onclick={onClose}>Cancel</button>
      {/if}
      <button
        type="button"
        class="primary"
        onclick={() => void handleSave()}
        disabled={libreofficeSettingsStore.isSaving}
      >
        {libreofficeSettingsStore.isSaving ? 'Saving...' : 'Save Settings'}
      </button>
    </div>
  </div>

  {#if saveError}
    <p class="error">{saveError}</p>
  {:else if libreofficeSettingsStore.saveMessage}
    <p class="ok">{libreofficeSettingsStore.saveMessage}</p>
  {/if}
</div>

<style>
  .settings-page {
    display: grid;
    gap: 1rem;
  }

  .settings-section {
    border: 1px solid #334155;
    border-radius: 10px;
    padding: 1rem;
    background: #0b1220;
  }

  h3 {
    margin-top: 0;
    margin-bottom: 0.9rem;
    color: #7dd3fc;
  }

  label {
    display: block;
    font-weight: 700;
    margin-top: 0.7rem;
    margin-bottom: 0.3rem;
    color: #cbd5e1;
  }

  input,
  select,
  textarea {
    width: 100%;
    border: 1px solid #334155;
    border-radius: 8px;
    background: #020617;
    color: #e2e8f0;
    padding: 0.6rem 0.7rem;
    font: inherit;
  }

  .footer {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .actions {
    display: flex;
    gap: 0.6rem;
  }

  .primary,
  .secondary {
    border-radius: 8px;
    padding: 0.6rem 0.9rem;
    font-weight: 700;
    cursor: pointer;
  }

  .primary {
    border: 1px solid #0ea5e9;
    background: #0ea5e9;
    color: #082f49;
  }

  .secondary {
    border: 1px solid #334155;
    background: #0f172a;
    color: #e2e8f0;
  }
</style>
