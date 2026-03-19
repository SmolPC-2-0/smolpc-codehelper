<script lang="ts">
  import type { IntegrationIssueReport } from '../types/libreoffice';

  type Props = {
    actionBusy: boolean;
    selectedModelId: string;
    issueRequestPayload: string;
    issueHttpStatus: string;
    issueResponseBody: string;
    integrationIssueReport: IntegrationIssueReport | null;
    evidenceExportPath: string | null;
    onIssueRequestPayloadChange: (nextValue: string) => void;
    onIssueHttpStatusChange: (nextValue: string) => void;
    onIssueResponseBodyChange: (nextValue: string) => void;
    onCreateIssueReport: () => void;
    onExportEvidenceBundle: () => void;
    onCopyIssueReport: () => void;
  };

  let {
    actionBusy,
    selectedModelId,
    issueRequestPayload,
    issueHttpStatus,
    issueResponseBody,
    integrationIssueReport,
    evidenceExportPath,
    onIssueRequestPayloadChange,
    onIssueHttpStatusChange,
    onIssueResponseBodyChange,
    onCreateIssueReport,
    onExportEvidenceBundle,
    onCopyIssueReport
  }: Props = $props();

  function handleIssueRequestPayloadInput(event: Event): void {
    const nextValue = (event.currentTarget as HTMLTextAreaElement | null)?.value ?? '';
    onIssueRequestPayloadChange(nextValue);
  }

  function handleIssueHttpStatusInput(event: Event): void {
    const nextValue = (event.currentTarget as HTMLInputElement | null)?.value ?? '';
    onIssueHttpStatusChange(nextValue);
  }

  function handleIssueResponseBodyInput(event: Event): void {
    const nextValue = (event.currentTarget as HTMLTextAreaElement | null)?.value ?? '';
    onIssueResponseBodyChange(nextValue);
  }
</script>

<section class="panel">
  <h2>Integration Issue Report</h2>
  <p class="muted">
    Generates the onboarding issue payload with app/version, OS/hardware summary, request/response payload,
    engine status/meta snapshots, and runtime override flags.
  </p>
  <div class="row stacked">
    <label for="issue-request-payload">Request Payload (JSON)</label>
    <textarea
      id="issue-request-payload"
      value={issueRequestPayload}
      rows="5"
      disabled={actionBusy}
      oninput={handleIssueRequestPayloadInput}
    ></textarea>
  </div>
  <div class="row">
    <label for="issue-http-status">HTTP Status</label>
    <input
      id="issue-http-status"
      type="text"
      value={issueHttpStatus}
      placeholder="e.g. 429"
      disabled={actionBusy}
      oninput={handleIssueHttpStatusInput}
    />
  </div>
  <div class="row stacked">
    <label for="issue-response-body">Response Body (text)</label>
    <textarea
      id="issue-response-body"
      value={issueResponseBody}
      rows="4"
      disabled={actionBusy}
      oninput={handleIssueResponseBodyInput}
    ></textarea>
  </div>
  <div class="actions">
    <button type="button" onclick={onCreateIssueReport} disabled={actionBusy}>
      Generate Issue Report
    </button>
    <button type="button" onclick={onExportEvidenceBundle} disabled={actionBusy || !selectedModelId}>
      Export Evidence Bundle
    </button>
    <button type="button" onclick={onCopyIssueReport} disabled={actionBusy || !integrationIssueReport}>
      Copy JSON
    </button>
  </div>
  {#if evidenceExportPath}
    <p class="kv">Evidence file: <code>{evidenceExportPath}</code></p>
  {/if}
  {#if integrationIssueReport}
    <pre>{JSON.stringify(integrationIssueReport, null, 2)}</pre>
  {:else}
    <p class="muted">No issue report generated yet.</p>
  {/if}
</section>
