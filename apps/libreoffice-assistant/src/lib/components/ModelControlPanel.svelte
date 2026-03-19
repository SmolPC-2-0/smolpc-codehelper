<script lang="ts">
  import type { ModelDefinition } from '../types/libreoffice';

  type Props = {
    actionBusy: boolean;
    models: ModelDefinition[];
    selectedModelId: string;
    currentModelId: string | null;
    onSelectedModelIdChange: (nextValue: string) => void;
    onRefreshModels: () => void;
    onLoadSelectedModel: () => void;
    onUnloadCurrentModel: () => void;
  };

  let {
    actionBusy,
    models,
    selectedModelId,
    currentModelId,
    onSelectedModelIdChange,
    onRefreshModels,
    onLoadSelectedModel,
    onUnloadCurrentModel
  }: Props = $props();

  function handleModelChange(event: Event): void {
    const nextValue = (event.currentTarget as HTMLSelectElement | null)?.value ?? '';
    onSelectedModelIdChange(nextValue);
  }
</script>

<section class="panel">
  <h2>Model Control</h2>
  <div class="row">
    <label for="model-id">Model</label>
    <select id="model-id" value={selectedModelId} disabled={actionBusy} onchange={handleModelChange}>
      {#each models as model}
        <option value={model.id}>{model.name} ({model.id})</option>
      {/each}
    </select>
  </div>
  <div class="actions">
    <button type="button" onclick={onRefreshModels} disabled={actionBusy}>Refresh Models</button>
    <button type="button" onclick={onLoadSelectedModel} disabled={actionBusy || !selectedModelId}>
      Load Model
    </button>
    <button type="button" onclick={onUnloadCurrentModel} disabled={actionBusy}>Unload Model</button>
  </div>
  <p class="kv">Current model: <code>{currentModelId ?? 'none'}</code></p>
</section>
