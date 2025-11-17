<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import { benchmarkStore } from '$lib/stores/benchmark.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import type { BenchmarkProgress } from '$lib/stores/benchmark.svelte';

	interface Props {
		visible: boolean;
	}

	let { visible = $bindable() }: Props = $props();

	let benchmarksDir = $state<string>('');

	onMount(async () => {
		// Get benchmarks directory
		try {
			benchmarksDir = await invoke<string>('get_benchmarks_directory');
		} catch (error) {
			console.error('Failed to get benchmarks directory:', error);
		}

		// Listen for progress updates
		const unlistenProgress = await listen<BenchmarkProgress>('benchmark_progress', (event) => {
			benchmarkStore.updateProgress(event.payload);
		});

		// Listen for completion
		const unlistenComplete = await listen<string>('benchmark_complete', (event) => {
			benchmarkStore.complete(event.payload);
		});

		// Cleanup listeners on unmount
		return () => {
			unlistenProgress();
			unlistenComplete();
		};
	});

	async function runBenchmark() {
		try {
			benchmarkStore.start();

			await invoke('run_benchmark', {
				model: settingsStore.selectedModel,
				iterations: 3
			});

			// Success is handled by the event listener
		} catch (error) {
			console.error('Benchmark failed:', error);
			benchmarkStore.setError(error as string);
		}
	}

	function openBenchmarksFolder() {
		// Use Tauri's shell plugin to open the folder
		// For now, just show the path
		if (benchmarksDir) {
			alert(`Benchmarks folder: ${benchmarksDir}`);
		}
	}

	function closePanel() {
		visible = false;
	}
</script>

{#if visible}
	<div
		class="fixed bottom-4 right-4 z-50 w-96 rounded-lg border border-border bg-background p-4 shadow-lg"
	>
		<div class="mb-4 flex items-center justify-between">
			<h3 class="text-lg font-semibold">Benchmark Suite</h3>
			<button
				onclick={closePanel}
				class="text-muted-foreground hover:text-foreground"
				aria-label="Close benchmark panel"
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					class="h-5 w-5"
					viewBox="0 0 20 20"
					fill="currentColor"
				>
					<path
						fill-rule="evenodd"
						d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
						clip-rule="evenodd"
					/>
				</svg>
			</button>
		</div>

		<div class="space-y-4">
			<!-- Status Display -->
			<div class="rounded-md border p-3">
				<div class="flex items-center gap-2">
					<div
						class={`h-3 w-3 rounded-full ${benchmarkStore.isRunning ? 'animate-pulse bg-blue-500' : benchmarkStore.error ? 'bg-red-500' : benchmarkStore.lastResultPath ? 'bg-green-500' : 'bg-gray-400'}`}
					></div>
					<span class="text-sm font-medium">
						{#if benchmarkStore.isRunning}
							Running Benchmark...
						{:else if benchmarkStore.error}
							Error
						{:else if benchmarkStore.lastResultPath}
							Benchmark Complete
						{:else}
							Ready
						{/if}
					</span>
				</div>

				{#if benchmarkStore.progress}
					<div class="mt-2">
						<div class="mb-1 flex justify-between text-xs text-muted-foreground">
							<span>{benchmarkStore.progress.current_test}</span>
							<span>{benchmarkStore.progress.current}/{benchmarkStore.progress.total}</span>
						</div>
						<div class="h-2 w-full rounded-full bg-muted">
							<div
								class="h-2 rounded-full bg-blue-500 transition-all duration-300"
								style="width: {(benchmarkStore.progress.current /
									benchmarkStore.progress.total) *
									100}%"
							></div>
						</div>
					</div>
				{/if}

				{#if benchmarkStore.error}
					<p class="mt-2 text-xs text-red-500">{benchmarkStore.error}</p>
				{/if}

				{#if benchmarkStore.lastResultPath}
					<p class="mt-2 text-xs text-muted-foreground">
						Saved to: {benchmarkStore.lastResultPath}
					</p>
				{/if}
			</div>

			<!-- Current Model Info -->
			<div class="rounded-md bg-muted p-3">
				<p class="text-xs text-muted-foreground">Model</p>
				<p class="font-medium">{settingsStore.selectedModel}</p>
			</div>

			<!-- Actions -->
			<div class="flex gap-2">
				<button
					onclick={runBenchmark}
					disabled={benchmarkStore.isRunning}
					class="flex-1 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
				>
					{benchmarkStore.isRunning ? 'Running...' : 'Run Benchmark'}
				</button>

				{#if benchmarksDir}
					<button
						onclick={openBenchmarksFolder}
						class="rounded-md border px-4 py-2 text-sm font-medium hover:bg-muted"
						title="Open benchmarks folder"
					>
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-5 w-5"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"
							/>
						</svg>
					</button>
				{/if}
			</div>

			<!-- Help Text -->
			<p class="text-xs text-muted-foreground">
				Runs 36 tests (12 prompts × 3 iterations) to measure performance. Results exported
				to CSV for analysis.
			</p>
		</div>
	</div>
{/if}
