<script lang="ts">
  import type { GenerationMetrics } from '../types/libreoffice';

  type Props = {
    actionBusy: boolean;
    streaming: boolean;
    prompt: string;
    generatedText: string;
    streamingText: string;
    lastMetrics: GenerationMetrics | null;
    onPromptChange: (nextValue: string) => void;
    onGenerateNonStream: () => void;
    onGenerateStream: () => void;
    onCancelGeneration: () => void;
  };

  let {
    actionBusy,
    streaming,
    prompt,
    generatedText,
    streamingText,
    lastMetrics,
    onPromptChange,
    onGenerateNonStream,
    onGenerateStream,
    onCancelGeneration
  }: Props = $props();

  function handlePromptInput(event: Event): void {
    const nextValue = (event.currentTarget as HTMLTextAreaElement | null)?.value ?? '';
    onPromptChange(nextValue);
  }
</script>

<section class="panel">
  <h2>Generation</h2>
  <div class="row stacked">
    <label for="prompt">Prompt</label>
    <textarea
      id="prompt"
      value={prompt}
      rows="5"
      disabled={actionBusy && !streaming}
      oninput={handlePromptInput}
    ></textarea>
  </div>
  <div class="actions">
    <button type="button" onclick={onGenerateNonStream} disabled={actionBusy || !prompt.trim()}>
      Generate (Non-Stream)
    </button>
    <button type="button" onclick={onGenerateStream} disabled={actionBusy || !prompt.trim()}>
      Generate (Stream)
    </button>
    <button type="button" onclick={onCancelGeneration} disabled={!streaming}>
      Cancel
    </button>
  </div>
  <div class="output-grid">
    <div>
      <h3>Output</h3>
      <pre>{generatedText || '(none yet)'}</pre>
    </div>
    <div>
      <h3>Streaming Buffer</h3>
      <pre>{streamingText || '(no stream chunks yet)'}</pre>
    </div>
  </div>
  {#if lastMetrics}
    <p class="kv">
      Metrics:
      <code>
        tokens={lastMetrics.total_tokens},
        ttft_ms={lastMetrics.time_to_first_token_ms ?? 'n/a'},
        tps={lastMetrics.tokens_per_second.toFixed(2)},
        total_ms={lastMetrics.total_time_ms}
      </code>
    </p>
  {/if}
</section>
