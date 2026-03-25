<script lang="ts">
	import { onMount } from 'svelte';
	import { provisioningStore, type ModelSource } from '$lib/stores/provisioning.svelte';
	import ProgressPanel from './ProgressPanel.svelte';
	import SourceSelector from './SourceSelector.svelte';

	interface Props {
		oncomplete: () => void;
	}

	let { oncomplete }: Props = $props();

	onMount(() => {
		provisioningStore.detectSources();
	});

	$effect(() => {
		if (provisioningStore.phase === 'complete') {
			oncomplete();
		}
	});

	function handleSelectSource(source: ModelSource) {
		const modelIds = provisioningStore.recommendation
			? [provisioningStore.recommendation.model_id]
			: [];
		provisioningStore.startProvisioning(source, modelIds);
	}

	function handleCancel() {
		provisioningStore.cancel();
	}

	function handleRetry() {
		provisioningStore.detectSources();
	}
</script>

<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-zinc-900"
	aria-label="First-run setup"
>
	<div class="flex w-full max-w-md flex-col gap-6 px-6">
		<!-- Header -->
		<div class="text-center">
			<h1 class="text-2xl font-bold text-zinc-100">SmolPC Code Helper</h1>
			<p class="mt-1 text-sm text-zinc-400">AI model setup</p>
		</div>

		<!-- Phase content -->
		{#if provisioningStore.phase === 'detecting'}
			<div class="flex flex-col items-center gap-3 py-8">
				<div
					class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-600 border-t-blue-500"
					role="status"
					aria-label="Detecting AI models"
				></div>
				<p class="text-sm text-zinc-400">Checking for AI models…</p>
			</div>
		{:else if provisioningStore.phase === 'ready'}
			<div class="flex flex-col gap-4">
				<div>
					<h2 class="text-lg font-semibold text-zinc-100">Choose installation source</h2>
					<p class="mt-1 text-sm text-zinc-400">Select how you want to install the AI model.</p>
				</div>
				<SourceSelector
					sources={provisioningStore.sources}
					recommendation={provisioningStore.recommendation}
					onselect={handleSelectSource}
					onretry={handleRetry}
					onskip={oncomplete}
				/>
			</div>
		{:else if provisioningStore.phase === 'provisioning' || provisioningStore.phase === 'verifying'}
			<div class="flex flex-col gap-4">
				<div>
					<h2 class="text-lg font-semibold text-zinc-100">Installing AI model</h2>
					<p class="mt-1 text-sm text-zinc-400">
						This may take a few minutes. Please keep the app open.
					</p>
				</div>
				<ProgressPanel
					archiveName={provisioningStore.currentArchive}
					bytesDown={provisioningStore.bytesDown}
					totalBytes={provisioningStore.totalBytes}
					phase={provisioningStore.phase}
					oncancel={handleCancel}
				/>
			</div>
		{:else if provisioningStore.phase === 'error'}
			<div class="flex flex-col gap-4 rounded-xl border border-rose-500/30 bg-rose-500/10 p-6">
				<div class="flex items-start gap-3">
					<div class="mt-0.5 h-5 w-5 shrink-0 text-rose-400">
						<svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
							<path
								fill-rule="evenodd"
								d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-5a.75.75 0 01.75.75v4.5a.75.75 0 01-1.5 0v-4.5A.75.75 0 0110 5zm0 10a1 1 0 100-2 1 1 0 000 2z"
								clip-rule="evenodd"
							></path>
						</svg>
					</div>
					<div class="min-w-0">
						<p class="text-sm font-semibold text-rose-300">Setup failed</p>
						<p class="mt-1 text-sm break-words text-rose-200/80">
							{provisioningStore.errorMessage || 'An unexpected error occurred.'}
						</p>
					</div>
				</div>
				<div class="flex gap-3">
					{#if provisioningStore.errorRetryable}
						<button
							type="button"
							onclick={handleRetry}
							class="rounded-lg border border-rose-500/40 bg-rose-500/20 px-4 py-2 text-sm font-medium text-rose-200 hover:bg-rose-500/30 focus:ring-2 focus:ring-rose-500 focus:ring-offset-2 focus:ring-offset-zinc-900 focus:outline-none"
						>
							Try again
						</button>
					{/if}
					<button
						type="button"
						onclick={oncomplete}
						class="rounded-lg px-4 py-2 text-sm text-zinc-500 hover:text-zinc-300"
					>
						Skip for now
					</button>
				</div>
			</div>
		{:else if provisioningStore.phase === 'complete'}
			<!-- Auto-transitions via $effect; show brief confirmation in case of delay -->
			<div class="flex flex-col items-center gap-3 py-8">
				<div
					class="flex h-10 w-10 items-center justify-center rounded-full bg-green-500/20 text-green-400"
				>
					<svg viewBox="0 0 20 20" fill="currentColor" class="h-6 w-6" aria-hidden="true">
						<path
							fill-rule="evenodd"
							d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
							clip-rule="evenodd"
						></path>
					</svg>
				</div>
				<p class="text-sm text-zinc-300">Model installed — launching app…</p>
			</div>
		{/if}
	</div>
</div>
