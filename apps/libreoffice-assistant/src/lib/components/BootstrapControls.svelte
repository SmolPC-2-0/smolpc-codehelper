<script lang="ts">
  type Props = {
    loadingBootstrap: boolean;
    actionBusy: boolean;
    commandError: string | null;
    actionMessage: string | null;
    onRefreshBootstrapStatus: () => void;
    onEnsureEngineStarted: () => void;
    onRefreshReadiness: () => void;
  };

  let {
    loadingBootstrap,
    actionBusy,
    commandError,
    actionMessage,
    onRefreshBootstrapStatus,
    onEnsureEngineStarted,
    onRefreshReadiness
  }: Props = $props();
</script>

<div class="actions">
  <button type="button" onclick={onRefreshBootstrapStatus} disabled={loadingBootstrap || actionBusy}>
    {loadingBootstrap ? 'Refreshing...' : 'Refresh Bootstrap'}
  </button>
  <button type="button" onclick={onEnsureEngineStarted} disabled={actionBusy || loadingBootstrap}>
    {actionBusy ? 'Working...' : 'Ensure Engine Started'}
  </button>
  <button type="button" onclick={onRefreshReadiness} disabled={actionBusy}>
    Refresh Readiness
  </button>
</div>

{#if commandError}
  <p class="error">{commandError}</p>
{/if}
{#if actionMessage}
  <p class="ok">{actionMessage}</p>
{/if}
