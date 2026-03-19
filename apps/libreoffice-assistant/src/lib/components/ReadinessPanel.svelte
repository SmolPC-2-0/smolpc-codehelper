<script lang="ts">
  import type { CheckModelResponse } from '../types/libreoffice';

  type Props = {
    readiness: CheckModelResponse | null;
  };

  let { readiness }: Props = $props();
</script>

<section class="panel">
  <h2>Readiness</h2>
  {#if readiness}
    <div class="lane-grid">
      <div>
        <h3>openvino_npu</h3>
        <p><code>ready={readiness.lanes.openvino_npu.ready ? 'true' : 'false'}</code></p>
        <p><code>reason={readiness.lanes.openvino_npu.reason}</code></p>
      </div>
      <div>
        <h3>directml</h3>
        <p><code>ready={readiness.lanes.directml.ready ? 'true' : 'false'}</code></p>
        <p><code>reason={readiness.lanes.directml.reason}</code></p>
      </div>
      <div>
        <h3>cpu</h3>
        <p><code>ready={readiness.lanes.cpu.ready ? 'true' : 'false'}</code></p>
        <p><code>reason={readiness.lanes.cpu.reason}</code></p>
      </div>
    </div>
  {:else}
    <p class="muted">No readiness snapshot yet.</p>
  {/if}
</section>
