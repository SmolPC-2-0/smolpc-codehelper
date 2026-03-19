import {
  DEFAULT_SOURCE_PARITY_SETTINGS,
  type SourceParitySettings,
  type SourceParityTheme,
  type SourceParityWorkflowMode
} from '../types/sourceParity';
import { loadFromStorage, saveToStorage } from '../utils/storage';

const STORAGE_KEY = 'libreoffice_assistant_source_parity_settings_v1';

function normalizeSettings(raw: Partial<SourceParitySettings>): SourceParitySettings {
  const normalizedTheme: SourceParityTheme =
    raw.theme === 'light' || raw.theme === 'dark' ? raw.theme : DEFAULT_SOURCE_PARITY_SETTINGS.theme;
  const normalizedWorkflowMode: SourceParityWorkflowMode =
    raw.workflow_mode === 'tool_first' || raw.workflow_mode === 'mcp_assisted'
      ? raw.workflow_mode
      : DEFAULT_SOURCE_PARITY_SETTINGS.workflow_mode;
  const normalizedTemperature = Number.isFinite(raw.temperature)
    ? Math.max(0, Math.min(2, Number(raw.temperature)))
    : DEFAULT_SOURCE_PARITY_SETTINGS.temperature;
  const normalizedMaxTokens = Number.isFinite(raw.max_tokens)
    ? Math.max(16, Math.min(8192, Math.floor(Number(raw.max_tokens))))
    : DEFAULT_SOURCE_PARITY_SETTINGS.max_tokens;

  return {
    selected_model:
      typeof raw.selected_model === 'string' && raw.selected_model.trim()
        ? raw.selected_model
        : DEFAULT_SOURCE_PARITY_SETTINGS.selected_model,
    python_path:
      typeof raw.python_path === 'string' && raw.python_path.trim()
        ? raw.python_path
        : DEFAULT_SOURCE_PARITY_SETTINGS.python_path,
    documents_path:
      typeof raw.documents_path === 'string' && raw.documents_path.trim()
        ? raw.documents_path
        : DEFAULT_SOURCE_PARITY_SETTINGS.documents_path,
    libreoffice_path:
      typeof raw.libreoffice_path === 'string' && raw.libreoffice_path.trim()
        ? raw.libreoffice_path
        : null,
    theme: normalizedTheme,
    system_prompt: typeof raw.system_prompt === 'string' ? raw.system_prompt : '',
    temperature: normalizedTemperature,
    max_tokens: normalizedMaxTokens,
    workflow_mode: normalizedWorkflowMode
  };
}

class LibreofficeSettingsStore {
  settings = $state<SourceParitySettings>({ ...DEFAULT_SOURCE_PARITY_SETTINGS });
  isLoading = $state(false);
  isSaving = $state(false);
  saveMessage = $state<string | null>(null);

  async loadSettings(): Promise<void> {
    this.isLoading = true;
    this.saveMessage = null;
    try {
      const loaded = loadFromStorage<Partial<SourceParitySettings>>(STORAGE_KEY, {});
      this.settings = normalizeSettings(loaded);
      this.applyTheme();
    } finally {
      this.isLoading = false;
    }
  }

  async saveSettings(): Promise<void> {
    this.isSaving = true;
    this.saveMessage = null;
    try {
      const success = saveToStorage(STORAGE_KEY, this.settings);
      if (!success) {
        throw new Error('Could not persist settings to localStorage.');
      }
      this.applyTheme();
      this.saveMessage = 'Settings saved.';
    } finally {
      this.isSaving = false;
    }
  }

  updateSetting<K extends keyof SourceParitySettings>(key: K, value: SourceParitySettings[K]): void {
    this.settings[key] = value;
    this.saveMessage = null;

    if (key === 'theme') {
      this.applyTheme();
    }
  }

  resetToDefaults(): void {
    this.settings = { ...DEFAULT_SOURCE_PARITY_SETTINGS };
    this.applyTheme();
    this.saveMessage = 'Settings reset to defaults. Save to keep changes.';
  }

  clearSaveMessage(): void {
    this.saveMessage = null;
  }

  applyTheme(): void {
    if (typeof window === 'undefined') {
      return;
    }

    document.documentElement.dataset.libreofficeTheme = this.settings.theme;
  }
}

export const libreofficeSettingsStore = new LibreofficeSettingsStore();
