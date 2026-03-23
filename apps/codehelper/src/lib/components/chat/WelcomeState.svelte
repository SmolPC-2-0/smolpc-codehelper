<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import QuickExamples, { type QuickExampleCard } from '$lib/components/QuickExamples.svelte';
	import type { AppMode } from '$lib/types/mode';

	interface Props {
		mode: AppMode;
		modeLabel: string;
		modeSubtitle: string;
		suggestions: string[];
		showQuickExamples: boolean;
		onSelectExample: (prompt: string) => void;
		onToggleExamples: (show: boolean) => void;
	}

	let { mode, modeLabel, modeSubtitle, suggestions, showQuickExamples, onSelectExample, onToggleExamples }: Props = $props();

	const MODE_COPY: Record<AppMode, { chip: string; headline: string; description: string }> = {
		code: {
			chip: 'Code Mode',
			headline: 'Ask for help with code, bugs, or new ideas.',
			description:
				'This mode is for reading code, explaining it clearly, finding bugs, and writing new functions. You can paste code or ask a plain-English question.'
		},
		gimp: {
			chip: 'GIMP Mode',
			headline: 'Edit the picture you have open in GIMP.',
			description:
				'This mode is for image edits. Ask to crop, brighten, clean up, rotate, or transform the image in plain language.'
		},
		blender: {
			chip: 'Blender Mode',
			headline: 'Ask Blender questions about the scene you have open.',
			description:
				'This mode helps with objects, modifiers, materials, and basic 3D workflows using the Blender scene as context.'
		},
		writer: {
			chip: 'Writer Mode',
			headline: 'Create and edit documents in Writer.',
			description:
				'This mode helps with writing, headings, tables, and document structure inside LibreOffice Writer.'
		},
		calc: {
			chip: 'Calc Preview',
			headline: 'Spreadsheet help is still being wired up.',
			description:
				'Calc is visible so users can see it is part of the unified product, but it should clearly look like a future mode for now.'
		},
		impress: {
			chip: 'Slides Mode',
			headline: 'Build or edit a presentation slide by slide.',
			description:
				'This mode helps create slide decks, add slides, change titles, and insert content into presentations.'
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

	const heroCopy = $derived(MODE_COPY[mode]);
	const examples = $derived<QuickExampleCard[]>(
		suggestions.map((prompt, index) => ({
			id: `${mode}-${index}`,
			title: buildExampleTitle(prompt),
			prompt
		}))
	);
</script>

<div class="welcome-state">
	<div class="welcome-state__hero">
		<div class="welcome-state__chip">
			<span>{heroCopy.chip}</span>
		</div>
		<h2>{heroCopy.headline}</h2>
		<p>{heroCopy.description}</p>
		<p class="welcome-state__subtitle">{modeLabel} · {modeSubtitle}</p>
	</div>

	<div class="welcome-state__examples">
		{#if showQuickExamples}
			<QuickExamples {examples} {onSelectExample} onClose={() => onToggleExamples(false)} />
		{:else}
			<Button variant="outline" onclick={() => onToggleExamples(true)}>Try an example</Button>
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
		font-size: 0.67rem;
		font-weight: 700;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: color-mix(in srgb, var(--color-primary) 58%, var(--color-foreground));
		border: 1px solid color-mix(in srgb, var(--color-primary) 16%, transparent);
		background: color-mix(in srgb, var(--brand-soft) 65%, transparent);
	}

	.welcome-state__hero h2 {
		font-size: clamp(1.5rem, 4vw, 2.3rem);
		line-height: 1.2;
		letter-spacing: -0.01em;
		font-weight: 620;
		max-width: 36rem;
	}

	.welcome-state__hero p {
		max-width: 40rem;
		color: var(--color-muted-foreground);
		font-size: 0.98rem;
	}

	.welcome-state__subtitle {
		color: var(--color-foreground);
		font-size: 0.88rem;
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
