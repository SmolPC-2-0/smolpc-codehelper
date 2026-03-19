<script lang="ts">
  import type { BackendStatus } from '../types/libreoffice';

  type Props = {
    backendStatus: BackendStatus | null;
  };

  let { backendStatus }: Props = $props();
</script>

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
