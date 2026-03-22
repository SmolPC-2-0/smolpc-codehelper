<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import type { SetupStatusDto } from '$lib/types/setup';

	interface Props {
		status: SetupStatusDto | null;
		error?: string | null;
		onOpen: () => void;
	}

	let { status, error = null, onOpen }: Props = $props();

	const headline = $derived(
		error
			? 'Setup check failed'
			: status?.overallState === 'error'
				? 'Setup needs repair'
				: 'Setup needs attention'
	);
	const detail = $derived(
		error ??
			status?.lastError ??
			status?.items.find((item) => item.state !== 'ready')?.detail ??
			'Bundled assets or host-app prerequisites still need review.'
	);
</script>

<aside class="setup-banner" aria-live="polite">
	<div class="setup-banner__copy">
		<span class="setup-banner__eyebrow">{headline}</span>
		<p>{detail}</p>
	</div>
	<Button variant="outline" onclick={onOpen}>Open setup</Button>
</aside>

<style>
	.setup-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.85rem;
		padding: 0.85rem 1rem;
		border-bottom: 1px solid color-mix(in srgb, var(--color-primary) 14%, var(--outline-soft));
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--brand-soft) 78%, transparent),
				color-mix(in srgb, var(--surface-widget) 96%, black)
			),
			var(--surface-widget);
	}

	.setup-banner__copy {
		display: grid;
		gap: 0.22rem;
		min-width: 0;
	}

	.setup-banner__eyebrow {
		font-size: 0.68rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: color-mix(in srgb, var(--color-primary) 68%, var(--color-foreground));
	}

	.setup-banner__copy p {
		font-size: 0.8rem;
		color: var(--color-muted-foreground);
	}

	@media (max-width: 720px) {
		.setup-banner {
			flex-direction: column;
			align-items: stretch;
		}
	}
</style>
