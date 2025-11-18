<script lang="ts">
	import { Button } from '$lib/components/ui/button';

	interface Props {
		onNavigate: (route: 'home' | 'codehelper' | 'libreoffice' | 'blender') => void;
	}

	let { onNavigate }: Props = $props();
</script>

<div class="flex h-screen flex-col bg-gray-50 dark:bg-gray-950">
	<!-- Header -->
	<header class="border-b border-gray-200 bg-white px-4 py-3 dark:border-gray-800 dark:bg-gray-900">
		<div class="flex items-center gap-3">
			<Button variant="ghost" onclick={() => onNavigate('home')}>
				← Home
			</Button>
			<h1 class="text-lg font-semibold text-gray-900 dark:text-white">
				Blender AI
			</h1>
		</div>
	</header>

	<!-- Content -->
	<div class="flex flex-1 items-center justify-center">
		<div class="text-center">
			<div class="mb-4 text-6xl">🎨</div>
			<h2 class="mb-2 text-3xl font-bold text-gray-900 dark:text-white">
				Blender AI
			</h2>
			<p class="mb-4 text-lg text-gray-600 dark:text-gray-400">
				Coming Soon
			</p>
			<p class="mb-8 max-w-md text-gray-500 dark:text-gray-500">
				AI-powered 3D modeling and rendering assistance will be available here.
			</p>
			<Button onclick={() => onNavigate('home')} variant="outline">
				Back to Home
			</Button>
		</div>
	</div>
</div>
