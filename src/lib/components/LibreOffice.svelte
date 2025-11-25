<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { libreOfficeStore } from '$lib/stores/libreoffice.svelte';
	import LibreOfficeStatus from '$lib/components/libreoffice/LibreOfficeStatus.svelte';
	import ToolList from '$lib/components/libreoffice/ToolList.svelte';
	import DocumentCreator from '$lib/components/libreoffice/DocumentCreator.svelte';
	import ToolExecutor from '$lib/components/libreoffice/ToolExecutor.svelte';
	import type { MCPTool } from '$lib/types/libreoffice';

	interface Props {
		onNavigate: (route: 'home' | 'codehelper' | 'libreoffice' | 'blender') => void;
	}

	let { onNavigate }: Props = $props();

	let selectedTool = $state<MCPTool | null>(null);
	let activeTab = $state<'create' | 'tools'>('create');

	function handleSelectTool(tool: MCPTool) {
		selectedTool = tool;
		activeTab = 'tools';
	}
</script>

<div class="flex h-screen flex-col bg-gray-50 dark:bg-gray-950">
	<!-- Header -->
	<header class="border-b border-gray-200 bg-white px-4 py-3 dark:border-gray-800 dark:bg-gray-900">
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				<Button variant="ghost" onclick={() => onNavigate('home')}>
					← Home
				</Button>
				<h1 class="text-lg font-semibold text-gray-900 dark:text-white">
					LibreOffice AI
				</h1>
			</div>
		</div>
	</header>

	<!-- Status bar -->
	<div class="border-b border-gray-200 bg-white px-4 py-2 dark:border-gray-800 dark:bg-gray-900">
		<LibreOfficeStatus
			status={libreOfficeStore.status}
			onConnect={() => libreOfficeStore.connect()}
			onDisconnect={() => libreOfficeStore.disconnect()}
		/>
	</div>

	<!-- Main content -->
	<div class="flex flex-1 overflow-hidden">
		<!-- Sidebar: Tool list -->
		<aside class="w-64 border-r border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-900">
			<div class="border-b border-gray-200 p-3 dark:border-gray-700">
				<h2 class="text-sm font-semibold text-gray-700 dark:text-gray-300">Available Tools</h2>
			</div>
			<div class="h-[calc(100%-48px)]">
				<ToolList
					tools={libreOfficeStore.tools}
					{selectedTool}
					onSelectTool={handleSelectTool}
				/>
			</div>
		</aside>

		<!-- Main panel -->
		<main class="flex-1 overflow-auto p-6">
			{#if !libreOfficeStore.isConnected}
				<!-- Not connected message -->
				<div class="flex h-full items-center justify-center">
					<div class="text-center">
						<div class="mb-4 text-6xl">📄</div>
						<h2 class="mb-2 text-2xl font-bold text-gray-900 dark:text-white">
							Connect to LibreOffice
						</h2>
						<p class="mb-6 max-w-md text-gray-600 dark:text-gray-400">
							Click "Connect" above to start using LibreOffice AI features.
							Make sure LibreOffice is running with the helper macro active.
						</p>
						<Button onclick={() => libreOfficeStore.connect()}>
							Connect Now
						</Button>
					</div>
				</div>
			{:else}
				<!-- Connected: Show tabs -->
				<div class="space-y-6">
					<!-- Tab buttons -->
					<div class="flex gap-2">
						<Button
							variant={activeTab === 'create' ? 'default' : 'outline'}
							onclick={() => (activeTab = 'create')}
						>
							Create Document
						</Button>
						<Button
							variant={activeTab === 'tools' ? 'default' : 'outline'}
							onclick={() => (activeTab = 'tools')}
						>
							Tool Executor
						</Button>
					</div>

					<!-- Tab content -->
					{#if activeTab === 'create'}
						<div class="max-w-md">
							<DocumentCreator
								disabled={!libreOfficeStore.isConnected}
								onCreateDocument={(filename, title, docType) =>
									libreOfficeStore.createDocument(filename, title, docType)
								}
							/>
						</div>
					{:else}
						<div class="max-w-2xl">
							<ToolExecutor
								tool={selectedTool}
								disabled={!libreOfficeStore.isConnected}
								onCallTool={(toolName, args) =>
									libreOfficeStore.callTool(toolName, args)
								}
							/>
						</div>
					{/if}
				</div>
			{/if}
		</main>
	</div>
</div>