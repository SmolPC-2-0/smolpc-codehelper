<script lang="ts">
	import type { InferenceStatus } from '$lib/types/inference';
	import HardwareIndicator from '$lib/components/HardwareIndicator.svelte';
	import StatusIndicator from '$lib/components/StatusIndicator.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Download, Keyboard, Menu } from '@lucide/svelte';

	interface Props {
		title: string;
		showSidebarToggle?: boolean;
		status: InferenceStatus;
		hardwareActive?: boolean;
		shortcutsOpen?: boolean;
		canExport?: boolean;
		onOpenSidebar: () => void;
		onToggleHardware: () => void;
		onToggleShortcuts: () => void;
		onExportChat: () => void;
	}

	let {
		title,
		showSidebarToggle = false,
		status,
		hardwareActive = false,
		shortcutsOpen = false,
		canExport = false,
		onOpenSidebar,
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
			<h1>{title}</h1>
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
		<StatusIndicator status={status} />
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
		min-width: 16rem;
		flex: 1;
	}

	.workspace-header__identity {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-width: 0;
	}

	.workspace-header__identity h1 {
		font-size: clamp(0.97rem, 2vw, 1.12rem);
		font-weight: 640;
		letter-spacing: 0.01em;
		color: var(--color-foreground);
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
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
