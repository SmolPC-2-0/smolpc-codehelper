<script lang="ts">
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { Laptop, MoonStar, Sun } from '@lucide/svelte';
	import type { AppSettings } from '$lib/types/settings';

	interface ThemeOption {
		value: AppSettings['theme'];
		label: string;
		icon: typeof Sun;
	}

	const options: ThemeOption[] = [
		{ value: 'system', label: 'System', icon: Laptop },
		{ value: 'light', label: 'Light', icon: Sun },
		{ value: 'dark', label: 'Dark', icon: MoonStar }
	];

	function handleThemeChange(theme: AppSettings['theme']) {
		settingsStore.setTheme(theme);
	}
</script>

<div class="control-pill">
	{#each options as option (option.value)}
		<button
			type="button"
			class={`theme-chip ${settingsStore.theme === option.value ? 'theme-chip--active' : ''}`}
			onclick={() => handleThemeChange(option.value)}
			aria-label={`Switch theme to ${option.label}`}
			title={`Theme: ${option.label}`}
		>
			<option.icon class="h-3.5 w-3.5" />
			<span>{option.label}</span>
		</button>
	{/each}
</div>

<style>
	.control-pill {
		display: inline-flex;
		gap: 0.375rem;
		border: 1px solid color-mix(in srgb, var(--color-border) 88%, transparent);
		border-radius: var(--radius-xl);
		padding: 0.25rem;
		background: var(--surface-widget);
		backdrop-filter: blur(8px);
		box-shadow: var(--shadow-soft);
	}

	.theme-chip {
		display: inline-flex;
		align-items: center;
		gap: 0.375rem;
		border: 1px solid transparent;
		border-radius: calc(var(--radius-lg) - 2px);
		padding: 0.35rem 0.6rem;
		color: var(--color-muted-foreground);
		background: transparent;
		font-size: 0.74rem;
		font-weight: 600;
		letter-spacing: 0.01em;
		cursor: pointer;
		transition:
			background var(--motion-fast),
			color var(--motion-fast),
			transform var(--motion-fast);
	}

	.theme-chip:hover {
		color: var(--color-foreground);
		background: var(--surface-hover);
		transform: translateY(-1px);
	}

	.theme-chip--active {
		border-color: color-mix(in srgb, var(--color-primary) 42%, var(--color-border));
		background: var(--surface-active);
		color: var(--color-foreground);
	}
</style>
