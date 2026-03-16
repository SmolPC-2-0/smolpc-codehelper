<script lang="ts">
	import { Layers3 } from '@lucide/svelte';
	import type { AppMode, ModeConfigDto } from '$lib/types/mode';

	interface Props {
		modes: ModeConfigDto[];
		activeMode: AppMode;
		onChange: (mode: AppMode) => void;
		disabled?: boolean;
	}

	let { modes, activeMode, onChange, disabled = false }: Props = $props();

	function handleChange(event: Event) {
		const target = event.target as HTMLSelectElement;
		onChange(target.value as AppMode);
	}
</script>

<div class="app-mode-dropdown">
	<Layers3 class="app-mode-dropdown__icon" />
	<select
		value={activeMode}
		onchange={handleChange}
		disabled={disabled}
		class="app-mode-dropdown__control"
		aria-label="Select assistant mode"
		title="Switch unified assistant mode"
	>
		{#each modes as mode (mode.id)}
			<option value={mode.id}>{mode.label}</option>
		{/each}
	</select>
</div>

<style>
	.app-mode-dropdown {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		min-width: 10.5rem;
		padding: 0.45rem 0.68rem;
		border-radius: var(--radius-xl);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		box-shadow: var(--glow-subtle);
	}

	:global(.app-mode-dropdown__icon) {
		width: 0.95rem;
		height: 0.95rem;
		color: var(--color-muted-foreground);
		flex-shrink: 0;
	}

	.app-mode-dropdown__control {
		flex: 1;
		border: none;
		outline: none;
		appearance: none;
		background: transparent;
		color: var(--color-foreground);
		font-size: 0.78rem;
		line-height: 1.25;
		padding-right: 0.4rem;
		color-scheme: light dark;
	}

	.app-mode-dropdown__control option {
		background: var(--surface-floating);
		color: var(--color-foreground);
	}

	.app-mode-dropdown__control:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	@media (max-width: 768px) {
		.app-mode-dropdown {
			min-width: 0;
			width: 100%;
		}
	}
</style>
