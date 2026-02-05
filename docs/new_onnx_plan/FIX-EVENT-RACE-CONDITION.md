# Plan: Fix Event Listener Race Condition in inference.svelte.ts

**Status**: Ready for implementation
**Created**: 2026-02-05

## Problem Summary

The `generateStream()` method in `src/lib/stores/inference.svelte.ts` has a race condition:

1. Sets up event listeners
2. Calls `await invoke('inference_generate', ...)` - resolves when Rust function returns
3. Immediately returns `lastMetrics` (still `null` because event hasn't arrived)
4. `finally` block cleans up listeners BEFORE `inference_done` event is processed

**Result**: Tokens may be lost, `isGenerating` may stay `true`, metrics not returned. This likely causes the "infinite token repetition" bug.

## Solution: Promise-Wrapper Pattern

Adopt the pattern from `src/main.js` (lines 362-477):

1. Set up ALL listeners FIRST
2. Create a Promise that resolves on `inference_done` / rejects on `inference_error`
3. Fire-and-forget `invoke()` (don't await it for completion)
4. AWAIT the completion Promise
5. Clean up listeners in `finally` AFTER Promise settles

## File Changes

**Single file**: `src/lib/stores/inference.svelte.ts`

### Remove (lines 26-32, 37-82)

- Module-level listener variables (`tokenUnlisten`, `doneUnlisten`, etc.)
- `streamCallback` variable
- `setupStreamListeners()` function
- `cleanupStreamListeners()` function

### Rewrite `generateStream()` method (lines 207-255)

```typescript
async generateStream(
    prompt: string,
    onToken: (token: string) => void,
    config?: Partial<GenerationConfig>
): Promise<GenerationMetrics | null> {
    if (!isLoaded) {
        error = 'No model loaded';
        return null;
    }
    if (isGenerating) {
        error = 'Generation already in progress';
        return null;
    }

    isGenerating = true;
    error = null;
    lastMetrics = null;

    const unlistenFns: UnlistenFn[] = [];

    try {
        // 1. Set up token listener FIRST
        const tokenUnlisten = await listen<string>('inference_token', (event) => {
            onToken(event.payload);
        });
        unlistenFns.push(tokenUnlisten);

        // 2. Create completion Promise
        const completionPromise = new Promise<GenerationMetrics | null>((resolve, reject) => {
            listen<GenerationMetrics>('inference_done', (event) => {
                lastMetrics = event.payload;
                isGenerating = false;
                resolve(event.payload);
            }).then((fn) => unlistenFns.push(fn));

            listen<string>('inference_error', (event) => {
                error = event.payload;
                isGenerating = false;
                reject(new Error(event.payload));
            }).then((fn) => unlistenFns.push(fn));

            listen('inference_cancelled', () => {
                isGenerating = false;
                resolve(null);
            }).then((fn) => unlistenFns.push(fn));
        });

        // 3. Build config
        const fullConfig: GenerationConfig | undefined = config
            ? {
                max_length: config.max_length ?? 2048,
                temperature: config.temperature ?? 0.7,
                top_k: config.top_k ?? 40,
                top_p: config.top_p ?? 0.9
            }
            : undefined;

        // 4. Start generation (fire-and-forget)
        invoke('inference_generate', { prompt, config: fullConfig }).catch((e) => {
            console.error('Invoke error:', e);
            error = String(e);
            isGenerating = false;
        });

        // 5. AWAIT completion
        return await completionPromise;

    } catch (e) {
        error = String(e);
        console.error('Streaming generation failed:', e);
        isGenerating = false;
        return null;

    } finally {
        // 6. Clean up AFTER completion
        for (const unlisten of unlistenFns) {
            unlisten?.();
        }
    }
}
```

## Key Differences

| Aspect | Before (Buggy) | After (Fixed) |
|--------|----------------|---------------|
| Listener storage | Module-level | Local array |
| When invoke "completes" | Await invoke | Fire-and-forget |
| When function returns | After invoke returns | After event Promise resolves |
| Cleanup timing | Immediately in finally | After Promise settles |

## API Compatibility

**No changes to public API**:
- Same signature: `generateStream(prompt, onToken, config?)`
- Same return type: `Promise<GenerationMetrics | null>`
- Same behavior for `isGenerating`, `error`, `lastMetrics` getters

## Verification

1. **Normal completion**: Send prompt → tokens stream → metrics returned → `isGenerating` = false
2. **Cancellation**: Start → cancel → returns `null` → `isGenerating` = false
3. **Error handling**: Unload model → attempt generate → error surfaced → `isGenerating` = false
4. **No listener leaks**: Generate twice → no duplicate events

## Related Issues

Other bugs identified that could be addressed later:
- **No repetition penalty** in `generator.rs` sampling (causes model loops)
- **Prompt format mismatch** - may not match Qwen2.5 chat template
- **Default temperature inconsistency** - Rust default is 1.0, frontend sends 0.7
