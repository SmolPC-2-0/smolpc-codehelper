<script lang="ts">
	import type { InferenceStatus } from '$lib/types/inference';
	import type { AppMode, ModeConfigDto } from '$lib/types/mode';
	import AppModeDropdown from '$lib/components/layout/AppModeDropdown.svelte';
	import HardwareIndicator from '$lib/components/HardwareIndicator.svelte';
	import StatusIndicator from '$lib/components/StatusIndicator.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Download, Keyboard, Menu } from '@lucide/svelte';

	interface Props {
		title: string;
		modeLabel: string;
		modeSubtitle: string;
		modeStatusLabel?: string | null;
		modeStatusDetail?: string | null;
		modes: ModeConfigDto[];
		activeMode: AppMode;
		showSidebarToggle?: boolean;
		status: InferenceStatus;
		modelInfoActive?: boolean;
		hardwareActive?: boolean;
		shortcutsOpen?: boolean;
		canExport?: boolean;
		onOpenSidebar: () => void;
		onChangeMode: (mode: AppMode) => void;
		onToggleModelInfo: () => void;
		onToggleHardware: () => void;
		onToggleShortcuts: () => void;
		onExportChat: () => void;
	}

	let {
		title,
		modeLabel,
		modeSubtitle,
		modeStatusLabel = null,
		modeStatusDetail = null,
		modes,
		activeMode,
		showSidebarToggle = false,
		status,
		modelInfoActive = false,
		hardwareActive = false,
		shortcutsOpen = false,
		canExport = false,
		onOpenSidebar,
		onChangeMode,
		onToggleModelInfo,
		onToggleHardware,
		onToggleShortcuts,
		onExportChat
	}: Props = $props();
</script>

<header class="workspace-header">
	<div class="workspace-header__main">
		<div class="workspace-header__identity">
			{#if showSidebarToggle}
				<Button
					variant="ghost"
					size="icon"
					onclick={onOpenSidebar}
					class="workspace-header__menu"
					aria-label="Open sidebar"
				>
					<Menu class="h-5 w-5" />
				</Button>
			{/if}

			<div class="workspace-header__copy">
				<div class="workspace-header__eyebrow-row">
					<span class="workspace-header__eyebrow">{modeLabel}</span>
					{#if modeStatusLabel}
						<span class="workspace-header__status-pill">{modeStatusLabel}</span>
					{/if}
				</div>
				<h1>{title}</h1>
				<p>{modeSubtitle}</p>
				{#if modeStatusDetail}
					<p class="workspace-header__detail">{modeStatusDetail}</p>
				{/if}
			</div>
		</div>

		<div class="workspace-header__selectors">
			<AppModeDropdown {modes} {activeMode} onChange={onChangeMode} />
		</div>
	</div>
	<div class="workspace-header__actions">
		<Button
			variant="ghost"
			size="icon"
			onclick={onExportChat}
			class="workspace-header__icon-button"
			aria-label="Export chat to markdown"
			title="Export current chat"
			disabled={!canExport}
		>
			<Download class="h-4 w-4" />
		</Button>
		<Button
			variant="ghost"
			size="icon"
			onclick={onToggleShortcuts}
			class={`workspace-header__icon-button ${shortcutsOpen ? 'workspace-header__icon-button--active' : ''}`}
			aria-label="Open keyboard shortcuts"
			title="Keyboard shortcuts (Ctrl/Cmd + /)"
		>
			<Keyboard class="h-4 w-4" />
		</Button>
		<HardwareIndicator onclick={onToggleHardware} active={hardwareActive} />
		<StatusIndicator {status} active={modelInfoActive} onToggle={onToggleModelInfo} />
	</div>
</header>

<style>
	.workspace-header {
		position: relative;
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		align-items: center;
		justify-content: space-between;
		padding: 1rem 1rem 0.8rem;
		border-bottom: 1px solid var(--outline-soft);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-elevated) 98%, black),
				var(--surface-subtle) 72%
			),
			var(--surface-subtle);
		backdrop-filter: blur(12px);
	}

	.workspace-header::after {
		content: '';
		position: absolute;
		inset-inline: 0;
		bottom: -1px;
		height: 1px;
		background: linear-gradient(
			90deg,
			transparent,
			color-mix(in srgb, var(--color-primary) 22%, transparent),
			transparent
		);
	}

	.workspace-header__main {
		display: flex;
		align-items: center;
		gap: 0.85rem;
		min-width: 18rem;
		flex: 1;
	}

	.workspace-header__identity {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-width: 0;
		flex: 1;
	}

	.workspace-header__copy {
		display: grid;
		gap: 0.18rem;
		min-width: 0;
	}

	.workspace-header__copy h1 {
		font-size: clamp(0.97rem, 2vw, 1.12rem);
		font-weight: 640;
		letter-spacing: 0.01em;
		color: var(--color-foreground);
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}

	.workspace-header__copy p {
		font-size: 0.75rem;
		color: var(--color-muted-foreground);
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}

	.workspace-header__detail {
		font-size: 0.72rem;
		color: color-mix(in srgb, var(--color-muted-foreground) 88%, var(--color-foreground));
	}

	.workspace-header__eyebrow-row {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.45rem;
		min-width: 0;
	}

	.workspace-header__eyebrow,
	.workspace-header__status-pill {
		display: inline-flex;
		align-items: center;
		padding: 0.18rem 0.48rem;
		border-radius: 999px;
		font-size: 0.63rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.workspace-header__eyebrow {
		color: color-mix(in srgb, var(--color-primary) 62%, var(--color-foreground));
		background: color-mix(in srgb, var(--brand-soft) 72%, transparent);
		border: 1px solid color-mix(in srgb, var(--color-primary) 18%, transparent);
	}

	.workspace-header__status-pill {
		color: var(--color-muted-foreground);
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
		border: 1px solid var(--outline-soft);
	}

	.workspace-header__selectors {
		flex-shrink: 0;
	}

	:global(.workspace-header__menu) {
		flex-shrink: 0;
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		box-shadow: var(--glow-subtle);
	}

	.workspace-header__actions {
		display: flex;
		align-items: center;
		gap: 0.6rem;
	}

	:global(.workspace-header__icon-button) {
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		box-shadow: var(--glow-subtle);
	}

	:global(.workspace-header__icon-button--active) {
		border-color: var(--outline-strong);
		background: var(--surface-active);
	}

	@media (max-width: 920px) {
		.workspace-header__main {
			flex-direction: column;
			align-items: stretch;
			min-width: 100%;
		}

		.workspace-header__selectors {
			width: 100%;
		}
	}

	@media (max-width: 720px) {
		.workspace-header {
			padding: 0.8rem;
		}

		.workspace-header__actions {
			width: 100%;
			justify-content: space-between;
		}

		.workspace-header__identity {
			width: 100%;
		}
	}
</style>
