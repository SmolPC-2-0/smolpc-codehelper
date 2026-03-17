<script lang="ts">
	import { onMount } from 'svelte';
	import { AlertCircle, RefreshCw, ShieldCheck, Wrench, X } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import type { SetupItemDto, SetupStatusDto } from '$lib/types/setup';

	interface Props {
		visible: boolean;
		status: SetupStatusDto | null;
		error?: string | null;
		loading?: boolean;
		preparing?: boolean;
		onRefresh: () => void;
		onPrepare: () => void;
		onClose?: () => void;
	}

	let {
		visible,
		status,
		error = null,
		loading = false,
		preparing = false,
		onRefresh,
		onPrepare,
		onClose
	}: Props = $props();

	onMount(() => {
		function handleKeydown(event: KeyboardEvent) {
			if (event.key === 'Escape' && visible) {
				onClose?.();
			}
		}

		window.addEventListener('keydown', handleKeydown);
		return () => window.removeEventListener('keydown', handleKeydown);
	});

	function stateLabel(item: SetupItemDto): string {
		return item.state.replaceAll('_', ' ');
	}
</script>

{#if visible}
	<div class="setup-panel">
		<div class="setup-panel__header">
			<h3 class="setup-panel__title">
				<Wrench class="h-4.5 w-4.5" />
				Setup & Provisioning
			</h3>
			<div class="setup-panel__actions">
				<Button
					variant="ghost"
					size="icon"
					onclick={onRefresh}
					disabled={loading || preparing}
					aria-label="Refresh setup status"
					title="Refresh setup status"
				>
					<RefreshCw class={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					onclick={() => onClose?.()}
					aria-label="Close setup panel"
				>
					<X class="h-4 w-4" />
				</Button>
			</div>
		</div>

		<div class="setup-panel__content">
			{#if error}
				<div class="setup-panel__error">
					<AlertCircle class="h-4 w-4" />
					<span>{error}</span>
				</div>
			{/if}

			<div class="setup-panel__summary">
				<div>
					<div class="setup-panel__summary-label">Overall state</div>
					<div class="setup-panel__summary-value">
						{status?.overallState?.replaceAll('_', ' ') ?? (error ? 'error' : 'loading')}
					</div>
				</div>
				<Button variant="outline" onclick={onPrepare} disabled={preparing}>
					{preparing ? 'Preparing…' : 'Prepare'}
				</Button>
			</div>

			<div class="setup-panel__items">
				{#each status?.items ?? [] as item (item.id)}
					<section class="setup-panel__item">
						<div class="setup-panel__item-top">
							<div>
								<div class="setup-panel__item-label">{item.label}</div>
								<div class="setup-panel__item-id">{item.id}</div>
							</div>
							<div class={`setup-panel__state setup-panel__state--${item.state}`}>
								{stateLabel(item)}
							</div>
						</div>
						{#if item.detail}
							<p class="setup-panel__item-detail">{item.detail}</p>
						{/if}
						<div class="setup-panel__item-meta">
							<span>{item.required ? 'Required' : 'Optional'}</span>
							<span>{item.canPrepare ? 'Prepare-enabled' : 'Prepare not needed here'}</span>
						</div>
					</section>
				{/each}
			</div>

			<div class="setup-panel__note">
				<ShieldCheck class="h-4 w-4" />
				<span>
					Phase 2 only validates manifests, prepares app-local setup state, and reports host-app
					detection. It does not launch or provision external host apps yet.
				</span>
			</div>
		</div>
	</div>
{/if}

<style>
	.setup-panel {
		position: fixed;
		right: 1rem;
		top: 5.75rem;
		z-index: 56;
		width: min(30rem, calc(100vw - 1.6rem));
		max-height: min(78vh, 44rem);
		overflow-y: auto;
		border-radius: calc(var(--radius-xl) + 6px);
		border: 1px solid var(--outline-soft);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-floating) 96%, black),
				color-mix(in srgb, var(--surface-subtle) 97%, black)
			),
			var(--surface-floating);
		box-shadow: var(--shadow-strong);
		backdrop-filter: blur(14px);
	}

	.setup-panel__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.85rem 0.9rem;
		border-bottom: 1px solid var(--outline-soft);
	}

	.setup-panel__title {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.95rem;
		font-weight: 700;
	}

	.setup-panel__actions {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
	}

	.setup-panel__content {
		display: grid;
		gap: 0.9rem;
		padding: 0.9rem;
	}

	.setup-panel__error,
	.setup-panel__note,
	.setup-panel__summary,
	.setup-panel__item {
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-lg);
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
	}

	.setup-panel__error,
	.setup-panel__note {
		display: flex;
		align-items: flex-start;
		gap: 0.55rem;
		padding: 0.75rem;
		font-size: 0.78rem;
		color: var(--color-muted-foreground);
	}

	.setup-panel__error {
		border-color: color-mix(in srgb, #d9534f 34%, var(--outline-soft));
	}

	.setup-panel__summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 0.8rem;
	}

	.setup-panel__summary-label,
	.setup-panel__item-id,
	.setup-panel__item-meta {
		font-size: 0.72rem;
		color: var(--color-muted-foreground);
	}

	.setup-panel__summary-value,
	.setup-panel__item-label {
		font-size: 0.9rem;
		font-weight: 650;
	}

	.setup-panel__items {
		display: grid;
		gap: 0.75rem;
	}

	.setup-panel__item {
		padding: 0.8rem;
		display: grid;
		gap: 0.5rem;
	}

	.setup-panel__item-top {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.75rem;
	}

	.setup-panel__state {
		display: inline-flex;
		align-items: center;
		padding: 0.22rem 0.55rem;
		border-radius: 999px;
		font-size: 0.68rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		border: 1px solid var(--outline-soft);
	}

	.setup-panel__state--ready {
		color: #1f7a3d;
		background: color-mix(in srgb, #1f7a3d 12%, transparent);
	}

	.setup-panel__state--missing,
	.setup-panel__state--not_prepared,
	.setup-panel__state--error {
		color: color-mix(in srgb, #c46b00 80%, var(--color-foreground));
		background: color-mix(in srgb, #c46b00 12%, transparent);
	}

	.setup-panel__state--error {
		color: #b83b36;
		background: color-mix(in srgb, #b83b36 12%, transparent);
	}

	.setup-panel__item-detail {
		font-size: 0.78rem;
		color: var(--color-foreground);
	}

	.setup-panel__item-meta {
		display: flex;
		flex-wrap: wrap;
		gap: 0.6rem;
	}
</style>
