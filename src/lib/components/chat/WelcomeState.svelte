<script lang="ts">
	import { Sparkles, Wand2 } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import QuickExamples from '$lib/components/QuickExamples.svelte';

	interface Props {
		showQuickExamples: boolean;
		onSelectExample: (prompt: string) => void;
		onToggleExamples: (show: boolean) => void;
	}

	let { showQuickExamples, onSelectExample, onToggleExamples }: Props = $props();
</script>

<div class="welcome-state">
	<div class="welcome-state__hero">
		<div class="welcome-state__chip">
			<Sparkles class="h-4 w-4" />
			<span>Offline AI Coding Lab</span>
		</div>
		<h2>Build, debug, and learn in one focused workspace.</h2>
		<p>
			SmolPC Code Helper is tuned for student-friendly explanations while keeping a serious
			developer workflow.
		</p>
	</div>

	<div class="welcome-state__examples">
		{#if showQuickExamples}
			<QuickExamples onSelectExample={onSelectExample} onClose={() => onToggleExamples(false)} />
		{:else}
			<Button variant="outline" onclick={() => onToggleExamples(true)}>
				<Wand2 class="mr-2 h-4 w-4" />
				Open Prompt Starters
			</Button>
		{/if}
	</div>
</div>

<style>
	.welcome-state {
		min-height: min(70vh, 46rem);
		display: grid;
		align-content: center;
		gap: 1.35rem;
		padding: 1.25rem 0.25rem;
	}

	.welcome-state__hero {
		display: grid;
		gap: 0.7rem;
		max-width: 54rem;
	}

	.welcome-state__chip {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		justify-self: start;
		padding: 0.35rem 0.6rem;
		border-radius: var(--radius-lg);
		font-size: 0.72rem;
		font-weight: 700;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: color-mix(in srgb, var(--color-primary) 72%, var(--color-foreground));
		border: 1px solid color-mix(in srgb, var(--color-primary) 22%, transparent);
		background: color-mix(in srgb, var(--color-primary) 10%, transparent);
	}

	.welcome-state__hero h2 {
		font-size: clamp(1.5rem, 4vw, 2.3rem);
		line-height: 1.2;
		letter-spacing: -0.02em;
		font-weight: 700;
		max-width: 36rem;
	}

	.welcome-state__hero p {
		max-width: 40rem;
		color: var(--color-muted-foreground);
		font-size: 0.98rem;
	}

	.welcome-state__examples {
		max-width: 58rem;
	}

	@media (max-width: 768px) {
		.welcome-state {
			min-height: 62vh;
			padding-top: 1rem;
		}
	}
</style>
