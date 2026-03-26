<script lang="ts">
	import { X } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import type { AppMode } from '$lib/types/mode';

	type ModeHelpContent = {
		summary: string;
		controls: string[];
		troubleshooting: string[];
	};

	interface Props {
		open: boolean;
		mode: AppMode;
		modeLabel: string;
		onClose: () => void;
	}

	let { open, mode, modeLabel, onClose }: Props = $props();

	const HELP_CONTENT: Record<AppMode, ModeHelpContent> = {
		code: {
			summary: 'Use Code mode when you want help understanding, fixing, or writing code.',
			controls: [
				'Mode changes which assistant toolchain you are using.',
				'Setup checks what the app needs to work and shows what needs attention.',
				'Model shows which AI model and runtime are currently active.',
				'Context decides whether recent chat history is included in answers.',
				'Prompt starters are quick ideas you can run or adapt.'
			],
			troubleshooting: [
				'If responses feel slow, ask a shorter question first.',
				'If generation stops, wait a moment and retry once.',
				'If startup or model info shows a problem, open Setup and review what needs attention.'
			]
		},
		gimp: {
			summary: 'Use GIMP mode when you want the assistant to help edit the image in GIMP.',
			controls: [
				'Mode keeps you in the GIMP workflow while still using the same chat shell.',
				'Open GIMP launches the host app when this mode is ready.',
				'Setup shows whether GIMP integration prerequisites are ready.',
				'Model and status controls still show system/runtime health.'
			],
			troubleshooting: [
				'Open GIMP first, then try your request again.',
				'If the mode shows unavailable, open Setup and fix the item marked for GIMP.',
				'If a request fails, retry with a simpler edit instruction (one change at a time).'
			]
		},
		blender: {
			summary: 'Use Blender mode when you want help with a live Blender scene.',
			controls: [
				'Mode switches the assistant to Blender-aware guidance.',
				'Open Blender launches the host app for scene-connected workflows.',
				'Setup shows whether Blender integration is ready.',
				'Model and status controls are still available for runtime checks.'
			],
			troubleshooting: [
				'Open Blender first so scene context is available.',
				'If Blender mode is unavailable, open Setup and resolve the Blender requirement.',
				'If answers seem off-topic, ask about one object or step at a time.'
			]
		},
		writer: {
			summary: 'Use Writer mode when you want help creating or editing a document.',
			controls: [
				'Mode routes the assistant to the Writer workflow.',
				'Open LibreOffice launches the host app for document actions.',
				'Setup shows whether LibreOffice integration is ready.',
				'Status controls still help diagnose model/runtime health.'
			],
			troubleshooting: [
				'Open LibreOffice Writer first, then retry.',
				'If Writer mode is unavailable, open Setup and check the LibreOffice requirement.',
				'If a request fails, use shorter instructions and mention the exact document change.'
			]
		},
		impress: {
			summary: 'Use Slides mode when you want help creating or editing a presentation.',
			controls: [
				'Mode switches the assistant to the Slides workflow (Impress backend).',
				'Open LibreOffice launches the host app for presentation tasks.',
				'Setup shows whether LibreOffice integration is ready.',
				'Model and status controls remain available for runtime visibility.'
			],
			troubleshooting: [
				'Open LibreOffice Impress first, then retry your request.',
				'If Slides mode is unavailable, open Setup and resolve LibreOffice prerequisites.',
				'If a request fails, ask for one slide action at a time.'
			]
		}
	};

	const content = $derived(HELP_CONTENT[mode]);
	const heading = $derived(`What can I do in ${modeLabel}?`);
</script>

{#if open}
	<div class="mode-help-drawer">
		<div class="mode-help-drawer__backdrop" onclick={onClose} aria-hidden="true"></div>
		<div
			class="mode-help-drawer__panel"
			role="dialog"
			aria-modal="true"
			aria-label="Mode help panel"
		>
			<header class="mode-help-drawer__header">
				<div class="mode-help-drawer__header-copy">
					<span class="mode-help-drawer__eyebrow">Help</span>
					<h2>{heading}</h2>
				</div>
				<Button variant="ghost" class="mode-help-drawer__close" onclick={onClose}>
					<X class="h-4 w-4" />
					<span>Close</span>
				</Button>
			</header>

			<div class="mode-help-drawer__content">
				<section class="mode-help-drawer__section">
					<h3>What this mode is for</h3>
					<p>{content.summary}</p>
				</section>

				<section class="mode-help-drawer__section">
					<h3>What the controls mean</h3>
					<ul>
						{#each content.controls as item (item)}
							<li>{item}</li>
						{/each}
					</ul>
				</section>

				<section class="mode-help-drawer__section">
					<h3>If something is not working</h3>
					<ul>
						{#each content.troubleshooting as item (item)}
							<li>{item}</li>
						{/each}
					</ul>
				</section>
			</div>
		</div>
	</div>
{/if}

<style>
	.mode-help-drawer {
		position: fixed;
		inset: 0;
		z-index: 82;
	}

	.mode-help-drawer__backdrop {
		position: absolute;
		inset: 0;
		background: rgb(3 7 16 / 56%);
		backdrop-filter: blur(3px);
	}

	.mode-help-drawer__panel {
		position: absolute;
		top: 0;
		right: 0;
		height: 100%;
		width: min(28rem, calc(100vw - 1rem));
		display: flex;
		flex-direction: column;
		border-left: 1px solid var(--outline-soft);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-floating) 98%, black),
				color-mix(in srgb, var(--surface-subtle) 96%, black)
			),
			var(--surface-floating);
		box-shadow: var(--shadow-strong);
		backdrop-filter: blur(12px);
	}

	.mode-help-drawer__header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 1rem 1rem 0.85rem;
		border-bottom: 1px solid var(--outline-soft);
	}

	.mode-help-drawer__header-copy {
		display: grid;
		gap: 0.35rem;
	}

	.mode-help-drawer__eyebrow {
		font-size: 0.68rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: color-mix(in srgb, var(--color-primary) 70%, var(--color-foreground));
	}

	.mode-help-drawer__header-copy h2 {
		font-size: 1rem;
		font-weight: 700;
		line-height: 1.3;
		color: var(--color-foreground);
	}

	:global(.mode-help-drawer__close) {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
	}

	.mode-help-drawer__content {
		flex: 1;
		overflow-y: auto;
		padding: 0.95rem 1rem 1.15rem;
		display: grid;
		align-content: start;
		gap: 0.75rem;
	}

	.mode-help-drawer__section {
		display: grid;
		gap: 0.5rem;
		padding: 0.8rem;
		border-radius: var(--radius-lg);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
	}

	.mode-help-drawer__section h3 {
		font-size: 0.84rem;
		font-weight: 700;
		color: var(--color-foreground);
	}

	.mode-help-drawer__section p,
	.mode-help-drawer__section li {
		font-size: 0.79rem;
		line-height: 1.45;
		color: var(--color-muted-foreground);
	}

	.mode-help-drawer__section ul {
		margin: 0;
		padding-left: 1rem;
		display: grid;
		gap: 0.35rem;
	}

	@media (max-width: 520px) {
		.mode-help-drawer__panel {
			width: calc(100vw - 0.5rem);
		}

		.mode-help-drawer__header,
		.mode-help-drawer__content {
			padding-left: 0.8rem;
			padding-right: 0.8rem;
		}
	}
</style>
