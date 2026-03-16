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
		statusLabel?: string | null;
		statusDetail?: string | null;
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
		statusLabel = null,
		statusDetail = null,
		showQuickExamples,
		disabledExamples = false,
		disabledReason = null,
		onSelectExample,
		onToggleExamples
	}: Props = $props();

	const MODE_COPY: Record<AppMode, { chip: string; headline: string; description: string }> = {
		code: {
			chip: 'Codehelper',
			headline: 'Fix bugs, write new code, and ask for clear explanations.',
			description:
				'Phase 3 keeps the real Codehelper generation path, model controls, and diagnostics active while the unified shell becomes its long-term home.'
		},
		gimp: {
			chip: 'GIMP Mode',
			headline: 'Edit the active image in GIMP from the unified assistant shell.',
			description:
				'GIMP is the first live external-provider mode. Ask for image edits, metadata, and supported drawing or transform actions directly from this chat.'
		},
		blender: {
			chip: 'Blender Mode',
			headline: 'Ask live Blender questions with scene-aware guidance from the unified shell.',
			description:
				'Blender mode is live in Phase 5. Ask about the current scene, modifiers, modeling workflows, or general Blender questions and the assistant will ground answers with scene context and local Blender reference docs when helpful.'
		},
		writer: {
			chip: 'Writer Mode',
			headline: 'Draft document assistance stays visible while LibreOffice wiring lands later.',
			description:
				'Writer is already present in the unified shell so the later LibreOffice provider can drop into a stable, reviewed UI.'
		},
		calc: {
			chip: 'Calc Mode',
			headline: 'Spreadsheet help gets a dedicated workspace before execution is enabled.',
			description:
				'Calc shares the future LibreOffice backend, but the shell currently only exposes its history, prompt starters, and provider status.'
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
		statusLabel ?? (providerState ? providerState.state.replace(/_/g, ' ') : 'status pending')
	);
	const heroCopy = $derived(MODE_COPY[mode]);
	const detailCopy = $derived(statusDetail ?? providerState?.detail ?? null);
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
		{#if detailCopy}
			<p class="welcome-state__detail">{detailCopy}</p>
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
			<Button variant="outline" onclick={() => onToggleExamples(true)}>Open Prompt Starters</Button>
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
