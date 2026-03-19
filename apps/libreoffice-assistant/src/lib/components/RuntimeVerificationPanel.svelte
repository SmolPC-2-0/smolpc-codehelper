<script lang="ts">
  import type { RuntimeVerificationReport } from '../types/libreoffice';

  type Props = {
    actionBusy: boolean;
    selectedModelId: string;
    runtimeVerification: RuntimeVerificationReport | null;
    onRunRuntimeChecklist: () => void;
  };

  let { actionBusy, selectedModelId, runtimeVerification, onRunRuntimeChecklist }: Props = $props();
</script>

<section class="panel">
  <h2>Runtime Verification</h2>
  <p class="muted">
    Runs contract-level checks aligned with <code>docs/APP_ONBOARDING_PLAYBOOK.md</code> against the selected
    model.
  </p>
  <div class="actions">
    <button type="button" onclick={onRunRuntimeChecklist} disabled={actionBusy || !selectedModelId}>
      Run Verification Checklist
    </button>
  </div>
  {#if runtimeVerification}
    <p class="kv">
      result:
      <code>{runtimeVerification.all_passed ? 'all_passed=true' : 'all_passed=false'}</code>
    </p>
    <div class="check-grid">
      {#each runtimeVerification.checks as check}
        <div class={check.ok ? 'check check-ok' : 'check check-fail'}>
          <p><code>{check.id}</code></p>
          <p>{check.detail}</p>
        </div>
      {/each}
    </div>
  {:else}
    <p class="muted">No runtime verification report yet.</p>
  {/if}
</section>
