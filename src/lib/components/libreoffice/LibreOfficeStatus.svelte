<script lang="ts">
	import type { LibreOfficeStatus } from '$lib/types/libreoffice';
	import { Button } from '$lib/components/ui/button';

	interface Props {
		status: LibreOfficeStatus;
		onConnect: () => void;
		onDisconnect: () => void;
	}

	let { status, onConnect, onDisconnect }: Props = $props();
</script>

<div class="flex items-center gap-3 rounded-lg border bg-white p-3 dark:border-gray-700 dark:bg-gray-800">
	<!-- Status indicator -->
	<div
		class={`h-3 w-3 rounded-full ${status.connected ? 'bg-green-500' : 'bg-red-500'} ${status.connecting ? 'animate-pulse' : ''}`}
	></div>

	<!-- Status text -->
	<div class="flex-1">
		<span class="text-sm font-medium text-gray-900 dark:text-white">
			{#if status.connecting}
				Connecting...
			{:else if status.connected}
				Connected
			{:else}
				Disconnected
			{/if}
		</span>
		{#if status.connected && status.serverName}
			<span class="ml-2 text-xs text-gray-500 dark:text-gray-400">
				{status.serverName} v{status.serverVersion}
			</span>
		{/if}
	</div>

	<!-- Error message -->
	{#if status.error}
		<span class="text-xs text-red-500">{status.error}</span>
	{/if}

	<!-- Connect/Disconnect button -->
	{#if status.connected}
		<Button variant="outline" size="sm" onclick={onDisconnect}>
			Disconnect
		</Button>
	{:else}
		<Button variant="default" size="sm" onclick={onConnect} disabled={status.connecting}>
			{status.connecting ? 'Connecting...' : 'Connect'}
		</Button>
	{/if}
</div>