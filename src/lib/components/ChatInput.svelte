<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Send } from '@lucide/svelte';

	interface Props {
		onSend: (message: string) => void;
		disabled?: boolean;
	}

	let { onSend, disabled = false }: Props = $props();

	let inputValue = $state('');

	function handleSubmit() {
		const trimmed = inputValue.trim();
		if (trimmed && !disabled) {
			onSend(trimmed);
			inputValue = '';
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSubmit();
		}
	}
</script>

<div class="flex gap-2">
	<Input
		bind:value={inputValue}
		onkeydown={handleKeydown}
		placeholder="Ask a coding question..."
		{disabled}
		class="flex-1"
	/>
	<Button onclick={handleSubmit} {disabled} size="icon">
		<Send class="h-4 w-4" />
	</Button>
</div>
