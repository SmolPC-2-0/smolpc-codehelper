<script lang="ts">
	import type { Message } from '$lib/types/ollama';
	import { Card, CardContent } from '$lib/components/ui/card';

	interface Props {
		message: Message;
	}

	let { message }: Props = $props();
</script>

<div class={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'} mb-4`}>
	<Card
		class={`max-w-[80%] ${message.role === 'user' ? 'bg-primary text-primary-foreground' : 'bg-muted'}`}
	>
		<CardContent class="p-4">
			<div class="mb-1 text-xs font-semibold opacity-70">
				{message.role === 'user' ? 'You' : 'Assistant'}
			</div>
			<div class="whitespace-pre-wrap break-words text-sm">
				{message.content}
				{#if message.isStreaming}
					<span class="inline-block h-4 w-1 animate-pulse bg-current"></span>
				{/if}
			</div>
		</CardContent>
	</Card>
</div>
