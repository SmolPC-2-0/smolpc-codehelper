<script lang="ts">
	import { Keyboard, X } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';

	interface ShortcutItem {
		keys: string[];
		description: string;
	}

	interface Props {
		open: boolean;
		onClose: () => void;
	}

	let { open, onClose }: Props = $props();

	const shortcuts: ShortcutItem[] = [
		{ keys: ['Enter'], description: 'Send message' },
		{ keys: ['Shift', 'Enter'], description: 'New line in composer' },
		{ keys: ['Ctrl/Cmd', 'Shift', 'B'], description: 'Toggle benchmark panel' },
		{ keys: ['Ctrl/Cmd', '\\'], description: 'Toggle sidebar collapse' },
		{ keys: ['Ctrl/Cmd', '/'], description: 'Toggle shortcuts overlay' },
		{ keys: ['Esc'], description: 'Close active panel/overlay' }
	];
</script>

{#if open}
	<div class="shortcuts-overlay">
		<div class="shortcuts-overlay__backdrop" onclick={onClose} aria-hidden="true"></div>
		<div class="shortcuts-overlay__panel" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
			<header class="shortcuts-overlay__header">
				<div class="shortcuts-overlay__title">
					<Keyboard class="h-4.5 w-4.5" />
					<h2>Keyboard Shortcuts</h2>
				</div>
				<Button variant="ghost" size="icon" onclick={onClose} aria-label="Close keyboard shortcuts">
					<X class="h-4 w-4" />
				</Button>
			</header>
			<div class="shortcuts-overlay__list">
				{#each shortcuts as shortcut}
					<div class="shortcuts-overlay__item">
						<div class="shortcuts-overlay__keys">
							{#each shortcut.keys as key}
								<kbd>{key}</kbd>
							{/each}
						</div>
						<p>{shortcut.description}</p>
					</div>
				{/each}
			</div>
		</div>
	</div>
{/if}

<style>
	.shortcuts-overlay {
		position: fixed;
		inset: 0;
		z-index: 80;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1rem;
	}

	.shortcuts-overlay__backdrop {
		position: absolute;
		inset: 0;
		background: rgb(3 7 16 / 64%);
		backdrop-filter: blur(4px);
	}

	.shortcuts-overlay__panel {
		position: relative;
		z-index: 1;
		width: min(34rem, 100%);
		border-radius: var(--radius-xl);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-floating) 96%, black);
		box-shadow: var(--shadow-strong);
		overflow: hidden;
		backdrop-filter: blur(12px);
	}

	.shortcuts-overlay__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.8rem 0.9rem;
		border-bottom: 1px solid var(--outline-soft);
	}

	.shortcuts-overlay__title {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
	}

	.shortcuts-overlay__title h2 {
		font-size: 0.95rem;
		font-weight: 700;
	}

	.shortcuts-overlay__list {
		display: grid;
		gap: 0.4rem;
		padding: 0.8rem;
	}

	.shortcuts-overlay__item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.7rem;
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-lg);
		padding: 0.55rem 0.6rem;
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
	}

	.shortcuts-overlay__keys {
		display: inline-flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.2rem;
	}

	.shortcuts-overlay__keys kbd {
		font-family: var(--font-code, 'JetBrains Mono', monospace);
		font-size: 0.68rem;
		padding: 0.2rem 0.34rem;
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-sm);
		background: color-mix(in srgb, var(--surface-hover) 72%, black);
	}

	.shortcuts-overlay__item p {
		font-size: 0.76rem;
		color: var(--color-muted-foreground);
		text-align: right;
	}

	@media (max-width: 620px) {
		.shortcuts-overlay__item {
			flex-direction: column;
			align-items: flex-start;
		}

		.shortcuts-overlay__item p {
			text-align: left;
		}
	}
</style>
