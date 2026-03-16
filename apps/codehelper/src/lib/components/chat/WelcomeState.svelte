<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import QuickExamples from '$lib/components/QuickExamples.svelte';
	import type { AppMode } from '$lib/types/mode';
	import type { ProviderStateDto } from '$lib/types/provider';

	interface Props {
		mode: AppMode;
		modeLabel: string;
		modeSubtitle: string;
		suggestions: string[];
		providerState?: ProviderStateDto | null;
		showQuickExamples: boolean;
		disabledExamples?: boolean;
		disabledReason?: string | null;
		onSelectExample: (prompt: string) => void;
		onToggleExamples: (show: boolean) => void;
	}

	let {
		mode,
		modeLabel,
		modeSubtitle,
		suggestions,
		providerState = null,
		showQuickExamples,
		disabledExamples = false,
		disabledReason = null,
		onSelectExample,
		onToggleExamples
	}: Props = $props();

	const MODE_COPY: Record<AppMode, { chip: string; headline: string; description: string }> = {
		code: {
			chip: 'Code Mode',
			headline: 'Build, debug, and explain code inside the unified shell.',
			description:
				'Phase 2 keeps the current Codehelper generation flow active while the rest of the unified frontend comes online.'
		},
		gimp: {
			chip: 'GIMP Mode',
			headline: 'Stage image-edit workflows before the tool bridge is wired.',
			description:
				'Use this shell pass to see the final GIMP mode layout, prompts, and provider status without enabling execution yet.'
		},
		blender: {
			chip: 'Blender Mode',
			headline: 'Preview scene-assistant workflows from the shared desktop shell.',
			description:
				'The Blender bridge is not connected in Phase 2, but the shell already reserves its mode identity and prompt surface.'
		},
		writer: {
			chip: 'Writer Mode',
			headline: 'Draft document assistance stays visible while LibreOffice wiring lands later.',
			description:
				'Writer is present in the unified shell now so the later LibreOffice provider can drop into a stable, reviewed UI.'
		},
		calc: {
			chip: 'Calc Mode',
			headline: 'Spreadsheet help gets a dedicated workspace before execution is enabled.',
			description:
				'Calc shares the same future LibreOffice backend, but Phase 2 only exposes the shell, history, and prompt starters.'
		},
		impress: {
			chip: 'Slides Mode',
			headline: 'Presentation support is visible now so the later provider can slot in cleanly.',
			description:
				'Slides is the user-facing label for Impress and stays read-only in the shell until the LibreOffice provider is integrated.'
		}
	};

	function buildExampleTitle(prompt: string): string {
		const trimmed = prompt.trim();
		const firstClause = trimmed.split(/[.!?]/)[0]?.trim() ?? trimmed;
		if (firstClause.length <= 30) {
			return firstClause;
		}

		return `${firstClause.slice(0, 27).trimEnd()}...`;
	}

	const examples = $derived(
		suggestions.map((prompt, index) => ({
			id: `${mode}-${index}`,
			title: buildExampleTitle(prompt),
			prompt
		}))
	);

	const providerLabel = $derived(
		providerState ? providerState.state.replace(/_/g, ' ') : 'status pending'
	);
	const heroCopy = $derived(MODE_COPY[mode]);
</script>

<div class="welcome-state">
	<div class="welcome-state__hero">
		<div class="welcome-state__chip-row">
			<div class="welcome-state__chip">
				<span>{heroCopy.chip}</span>
			</div>
			<div class="welcome-state__status-chip">
				<span>{providerLabel}</span>
			</div>
		</div>
		<h2>{heroCopy.headline}</h2>
		<p>{heroCopy.description}</p>
		<p class="welcome-state__subtitle">{modeLabel} · {modeSubtitle}</p>
		{#if providerState?.detail}
			<p class="welcome-state__detail">{providerState.detail}</p>
		{/if}
	</div>

	<div class="welcome-state__examples">
		{#if showQuickExamples}
			<QuickExamples
				{examples}
				{onSelectExample}
				onClose={() => onToggleExamples(false)}
				disabled={disabledExamples}
				{disabledReason}
			/>
		{:else}
			<Button variant="outline" onclick={() => onToggleExamples(true)}>
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

	.welcome-state__chip-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.55rem;
	}

	.welcome-state__chip,
	.welcome-state__status-chip {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		justify-self: start;
		padding: 0.35rem 0.6rem;
		border-radius: var(--radius-lg);
		font-size: 0.67rem;
		font-weight: 700;
		letter-spacing: 0.1em;
		text-transform: uppercase;
	}

	.welcome-state__chip {
		color: color-mix(in srgb, var(--color-primary) 58%, var(--color-foreground));
		border: 1px solid color-mix(in srgb, var(--color-primary) 16%, transparent);
		background: color-mix(in srgb, var(--brand-soft) 65%, transparent);
	}

	.welcome-state__status-chip {
		color: var(--color-muted-foreground);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
	}

	.welcome-state__hero h2 {
		font-size: clamp(1.5rem, 4vw, 2.3rem);
		line-height: 1.2;
		letter-spacing: -0.01em;
		font-weight: 620;
		max-width: 40rem;
	}

	.welcome-state__hero p {
		max-width: 42rem;
		color: var(--color-muted-foreground);
		font-size: 0.98rem;
	}

	.welcome-state__subtitle {
		color: var(--color-foreground);
		font-size: 0.88rem;
	}

	.welcome-state__detail {
		font-size: 0.84rem;
		color: color-mix(in srgb, var(--color-muted-foreground) 88%, var(--color-foreground));
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
