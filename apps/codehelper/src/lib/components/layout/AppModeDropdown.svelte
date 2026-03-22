<script lang="ts">
	import {
		ChevronDown,
		Code,
		Image,
		Box,
		FileText,
		Table,
		Presentation
	} from '@lucide/svelte';
	import type { AppMode, ModeConfigDto } from '$lib/types/mode';
	import type { Component } from 'svelte';

	interface Props {
		modes: ModeConfigDto[];
		activeMode: AppMode;
		onChange: (mode: AppMode) => void;
		disabled?: boolean;
		modeAvailability?: Record<string, boolean>;
		unavailableReasons?: Record<string, string>;
	}

	let {
		modes,
		activeMode,
		onChange,
		disabled = false,
		modeAvailability = {},
		unavailableReasons = {}
	}: Props = $props();

	let open = $state(false);
	let triggerEl: HTMLButtonElement | undefined = $state();
	let popoverEl: HTMLDivElement | undefined = $state();

	const ICON_MAP: Record<string, Component> = {
		code: Code,
		image: Image,
		box: Box,
		'file-text': FileText,
		table: Table,
		presentation: Presentation
	};

	const activeConfig = $derived(modes.find((m) => m.id === activeMode) ?? modes[0]);

	function isAvailable(modeId: string): boolean {
		if (modeId === 'code') return true;
		return modeAvailability[modeId] ?? true;
	}

	function toggleOpen() {
		if (disabled) return;
		open = !open;
	}

	function selectMode(modeId: AppMode) {
		if (!isAvailable(modeId)) return;
		onChange(modeId);
		open = false;
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			open = false;
			triggerEl?.focus();
		}
	}

	function handleClickOutside(event: MouseEvent) {
		if (!open) return;
		const target = event.target as Node;
		if (triggerEl?.contains(target) || popoverEl?.contains(target)) return;
		open = false;
	}

	$effect(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside, true);
			return () => document.removeEventListener('click', handleClickOutside, true);
		}
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="mode-dropdown" onkeydown={handleKeyDown}>
	<button
		bind:this={triggerEl}
		class="mode-dropdown__trigger"
		{disabled}
		onclick={toggleOpen}
		aria-haspopup="listbox"
		aria-expanded={open}
		aria-label="Select assistant mode"
	>
		{#if activeConfig}
			{@const IconComponent = ICON_MAP[activeConfig.icon]}
			{#if IconComponent}
				<IconComponent size={14} />
			{/if}
			<span class="mode-dropdown__trigger-label">{activeConfig.label}</span>
		{/if}
		<ChevronDown size={12} class="mode-dropdown__chevron" />
	</button>

	{#if open}
		<div bind:this={popoverEl} class="mode-dropdown__popover" role="listbox">
			{#each modes as mode (mode.id)}
				{@const available = isAvailable(mode.id)}
				{@const IconComponent = ICON_MAP[mode.icon]}
				<button
					class="mode-dropdown__option"
					class:active={mode.id === activeMode}
					class:unavailable={!available}
					disabled={!available}
					role="option"
					aria-selected={mode.id === activeMode}
					aria-disabled={!available}
					onclick={() => selectMode(mode.id as AppMode)}
				>
					{#if IconComponent}
						<span class="mode-dropdown__option-icon">
							<IconComponent size={14} />
						</span>
					{/if}
					<span class="mode-dropdown__option-text">
						<span class="mode-dropdown__option-label">{mode.label}</span>
						{#if !available && unavailableReasons[mode.id]}
							<span class="mode-dropdown__option-reason"
								>{unavailableReasons[mode.id]}</span
							>
						{/if}
					</span>
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.mode-dropdown {
		position: relative;
		display: inline-flex;
	}

	.mode-dropdown__trigger {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		min-width: 10.5rem;
		padding: 0.45rem 0.68rem;
		border-radius: var(--radius-xl);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		box-shadow: var(--glow-subtle);
		color: var(--color-foreground);
		font-size: 0.78rem;
		line-height: 1.25;
		cursor: pointer;
	}

	.mode-dropdown__trigger:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.mode-dropdown__trigger-label {
		flex: 1;
		text-align: left;
	}

	:global(.mode-dropdown__chevron) {
		color: var(--color-muted-foreground);
		flex-shrink: 0;
	}

	.mode-dropdown__popover {
		position: absolute;
		top: calc(100% + 4px);
		left: 0;
		z-index: 50;
		min-width: 100%;
		padding: 0.25rem;
		border-radius: var(--radius-xl);
		border: 1px solid var(--outline-soft);
		background: var(--surface-floating);
		box-shadow:
			0 4px 16px rgba(0, 0, 0, 0.24),
			0 1px 4px rgba(0, 0, 0, 0.12);
	}

	.mode-dropdown__option {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
		width: 100%;
		padding: 0.5rem 0.6rem;
		border: none;
		border-radius: calc(var(--radius-xl) - 2px);
		background: transparent;
		color: var(--color-foreground);
		font-size: 0.78rem;
		line-height: 1.35;
		cursor: pointer;
		text-align: left;
	}

	.mode-dropdown__option:hover:not(:disabled) {
		background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
	}

	.mode-dropdown__option.active {
		background: color-mix(in srgb, var(--color-foreground) 12%, transparent);
	}

	.mode-dropdown__option.unavailable {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.mode-dropdown__option-icon {
		flex-shrink: 0;
		margin-top: 1px;
		color: var(--color-muted-foreground);
	}

	.mode-dropdown__option-text {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
	}

	.mode-dropdown__option-label {
		font-weight: 500;
	}

	.mode-dropdown__option-reason {
		font-size: 0.68rem;
		color: var(--color-muted-foreground);
	}

	@media (max-width: 768px) {
		.mode-dropdown {
			width: 100%;
		}

		.mode-dropdown__trigger {
			min-width: 0;
			width: 100%;
		}
	}
</style>
