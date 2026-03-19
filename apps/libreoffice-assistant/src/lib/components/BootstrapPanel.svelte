<script lang="ts">
  import type { BootstrapStatus } from '../types/libreoffice';

  type Props = {
    bootstrap: BootstrapStatus | null;
  };

  let { bootstrap }: Props = $props();
</script>

{#if bootstrap}
  <section class="panel">
    <h2>Bootstrap Snapshot</h2>
    <dl>
      <div><dt>stage</dt><dd>{bootstrap.stage}</dd></div>
      <div><dt>healthy</dt><dd>{bootstrap.engine.healthy ? 'true' : 'false'}</dd></div>
      <div><dt>state</dt><dd>{bootstrap.engine.state ?? 'unknown'}</dd></div>
      <div><dt>active_backend</dt><dd>{bootstrap.engine.active_backend ?? 'none'}</dd></div>
      <div><dt>runtime_engine</dt><dd>{bootstrap.engine.runtime_engine ?? 'none'}</dd></div>
      <div><dt>selection_reason</dt><dd>{bootstrap.engine.selection_reason ?? 'none'}</dd></div>
      <div><dt>active_model_id</dt><dd>{bootstrap.engine.active_model_id ?? 'none'}</dd></div>
      <div><dt>engine_api_version</dt><dd>{bootstrap.engine.engine_api_version ?? 'unknown'}</dd></div>
      <div><dt>protocol_version</dt><dd>{bootstrap.engine.protocol_version ?? 'unknown'}</dd></div>
    </dl>
    {#if bootstrap.engine.error}
      <p class="error">{bootstrap.engine.error}</p>
    {/if}
  </section>

  <section class="panel">
    <h2>Phase Notes</h2>
    <ul>
      {#each bootstrap.notes as note}
        <li>{note}</li>
      {/each}
    </ul>
  </section>
{/if}
