<script lang="ts">
	import { Button } from '$lib/components/ui/button';

	interface Props {
		onNavigate: (route: 'codehelper' | 'libreoffice' | 'blender') => void;
	}

	let { onNavigate }: Props = $props();
</script>

<div class="flex h-screen flex-col items-center justify-center bg-gray-50 dark:bg-gray-950">
	<div class="text-center">
		<h1 class="mb-4 text-4xl font-bold text-gray-900 dark:text-white">
			Welcome to SmolPC
		</h1>
		<p class="mb-8 text-lg text-gray-600 dark:text-gray-400">
			Your offline AI learning tools
		</p>

		<div class="flex flex-col gap-4">
			<Button onclick={() => onNavigate('codehelper')} size="lg">
				💻 CodeHelper
			</Button>
			<Button onclick={() => onNavigate('libreoffice')} size="lg" variant="outline">
				📄 LibreOffice AI
			</Button>
			<Button onclick={() => onNavigate('blender')} size="lg" variant="outline" disabled>
				🎨 Blender AI (Coming Soon)
			</Button>
		</div>
	</div>
</div>
