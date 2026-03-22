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
	let focusedIndex = $state(-1);
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

	const POPOVER_ID = 'mode-dropdown-listbox';

	const activeConfig = $derived(modes.find((m) => m.id === activeMode) ?? modes[0]);

	function isAvailable(modeId: string): boolean {
		return modeAvailability[modeId] ?? true;
	}

	function toggleOpen() {
		if (disabled) return;
		open = !open;
		if (open) {
			focusedIndex = modes.findIndex((m) => m.id === activeMode);
		}
	}

	function selectMode(modeId: AppMode) {
		if (!isAvailable(modeId)) return;
		onChange(modeId);
		open = false;
		triggerEl?.focus();
	}

	function moveFocus(direction: 1 | -1) {
		if (!open) return;
		let next = focusedIndex + direction;
		// Wrap around
		if (next < 0) next = modes.length - 1;
		if (next >= modes.length) next = 0;
		focusedIndex = next;

		// Scroll the focused option into view
		const options = popoverEl?.querySelectorAll('[role="option"]');
		if (options?.[focusedIndex]) {
			(options[focusedIndex] as HTMLElement).scrollIntoView({ block: 'nearest' });
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		switch (event.key) {
			case 'Escape':
				open = false;
				triggerEl?.focus();
				event.preventDefault();
				break;
			case 'ArrowDown':
				if (!open) {
					open = true;
					focusedIndex = modes.findIndex((m) => m.id === activeMode);
				} else {
					moveFocus(1);
				}
				event.preventDefault();
				break;
			case 'ArrowUp':
				if (open) {
					moveFocus(-1);
				}
				event.preventDefault();
				break;
			case 'Enter':
			case ' ':
				if (open && focusedIndex >= 0 && focusedIndex < modes.length) {
					selectMode(modes[focusedIndex].id as AppMode);
					event.preventDefault();
				}
				break;
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
		aria-controls={POPOVER_ID}
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
		<div
			bind:this={popoverEl}
			id={POPOVER_ID}
			class="mode-dropdown__popover"
			role="listbox"
			aria-label="Assistant modes"
		>
			{#each modes as mode, index (mode.id)}
				{@const available = isAvailable(mode.id)}
				{@const IconComponent = ICON_MAP[mode.icon]}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					class="mode-dropdown__option"
					class:active={mode.id === activeMode}
					class:unavailable={!available}
					class:focused={index === focusedIndex}
					role="option"
					tabindex="-1"
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
				</div>
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

	.mode-dropdown__option:hover:not(.unavailable) {
		background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
	}

	.mode-dropdown__option.focused:not(.unavailable) {
		background: color-mix(in srgb, var(--color-foreground) 10%, transparent);
		outline: 1px solid var(--outline-soft);
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
