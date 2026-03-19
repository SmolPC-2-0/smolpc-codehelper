<script lang="ts">
  import type { SourceParityDependencyItem } from '../types/sourceParity';

  interface Props {
    loading: boolean;
    dependencies: SourceParityDependencyItem[];
    actionBusy: boolean;
    onRefreshChecks: () => void;
    onEnsureEngineStarted: () => void;
    onStartMcpServer: () => void;
  }

  let {
    loading,
    dependencies,
    actionBusy,
    onRefreshChecks,
    onEnsureEngineStarted,
    onStartMcpServer
  }: Props = $props();

  function statusLabel(status: SourceParityDependencyItem['status']): string {
    if (status === 'ready') {
      return 'Ready';
    }
    if (status === 'blocked') {
      return 'Blocked';
    }
    if (status === 'warning') {
      return 'Warning';
    }
    return 'Checking...';
  }
</script>

<div class="loading-screen">
  <div class="loading-header">
    <h3>Preparing Source-Parity Workspace</h3>
    <p>
      {loading
        ? 'Refreshing engine, model, and MCP status...'
        : 'Resolve blocked checks to unlock source-parity chat.'}
    </p>
  </div>

  <div class="dependency-list">
    {#each dependencies as dependency (dependency.key)}
      <div class="dependency-row">
        <div class="dependency-label">
          <span>{dependency.label}</span>
          {#if dependency.detail}
            <small>{dependency.detail}</small>
          {/if}
        </div>
        <span class={`badge ${dependency.status}`}>{statusLabel(dependency.status)}</span>
      </div>
    {/each}
  </div>

  <div class="actions">
    <button type="button" class="secondary" onclick={onRefreshChecks} disabled={actionBusy}>
      Refresh Checks
    </button>
    <button type="button" class="secondary" onclick={onEnsureEngineStarted} disabled={actionBusy}>
      Ensure Engine
    </button>
    <button type="button" class="primary" onclick={onStartMcpServer} disabled={actionBusy}>
      Start MCP
    </button>
  </div>
</div>

<style>
  .loading-screen {
    border: 1px solid #334155;
    border-radius: 10px;
    padding: 1rem;
    background: #0b1220;
    display: grid;
    gap: 0.9rem;
  }

  .loading-header h3 {
    margin: 0 0 0.45rem;
    color: #7dd3fc;
  }

  .loading-header p {
    margin: 0;
    color: #94a3b8;
  }

  .dependency-list {
    display: grid;
    gap: 0.55rem;
  }

  .dependency-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.8rem;
    border: 1px solid #334155;
    border-radius: 8px;
    background: #020617;
    padding: 0.7rem 0.75rem;
  }

  .dependency-label {
    display: grid;
    gap: 0.2rem;
    min-width: 0;
  }

  .dependency-label span {
    font-weight: 700;
    color: #e2e8f0;
  }

  .dependency-label small {
    color: #94a3b8;
    overflow-wrap: anywhere;
  }

  .badge {
    border-radius: 999px;
    padding: 0.3rem 0.65rem;
    font-size: 0.78rem;
    font-weight: 700;
    white-space: nowrap;
  }

  .badge.checking {
    background: #334155;
    color: #cbd5e1;
  }

  .badge.ready {
    background: #14532d;
    color: #bbf7d0;
  }

  .badge.warning {
    background: #78350f;
    color: #fde68a;
  }

  .badge.blocked {
    background: #7f1d1d;
    color: #fecaca;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.55rem;
  }

  .primary,
  .secondary {
    border-radius: 8px;
    padding: 0.5rem 0.8rem;
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

  button:disabled {
    opacity: 0.65;
    cursor: not-allowed;
  }
</style>
