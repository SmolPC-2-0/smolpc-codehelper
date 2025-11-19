import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';

// Global error handler to catch unhandled errors
window.addEventListener('error', (event) => {
	console.error('Global error caught:', event.error);
	document.body.innerHTML = `
		<div style="padding: 20px; font-family: sans-serif;">
			<h1>Application Error</h1>
			<p>An error occurred while loading the application:</p>
			<pre style="background: #f5f5f5; padding: 10px; border-radius: 4px; overflow: auto;">${event.error?.stack || event.error?.message || 'Unknown error'}</pre>
			<p>Please check the console for more details.</p>
		</div>
	`;
});

// Catch unhandled promise rejections
window.addEventListener('unhandledrejection', (event) => {
	console.error('Unhandled promise rejection:', event.reason);
});

try {
	mount(App, {
		target: document.getElementById('app')!
	});
} catch (error) {
	console.error('Failed to mount app:', error);
	document.body.innerHTML = `
		<div style="padding: 20px; font-family: sans-serif;">
			<h1>Failed to Start Application</h1>
			<p>The application failed to initialize:</p>
			<pre style="background: #f5f5f5; padding: 10px; border-radius: 4px; overflow: auto;">${error instanceof Error ? error.stack : String(error)}</pre>
			<p>Please check the console for more details.</p>
		</div>
	`;
	throw error;
}
