/**
 * Custom lightweight markdown renderer
 *
 * NOTE: For production use, install 'marked' package:
 * npm install marked
 *
 * Then replace this implementation with:
 * import { marked } from 'marked';
 * export function renderMarkdown(text: string): string {
 *   return marked(text);
 * }
 */

/**
 * Escape HTML to prevent XSS
 */
function escapeHtml(text: string): string {
	const map: Record<string, string> = {
		'&': '&amp;',
		'<': '&lt;',
		'>': '&gt;',
		'"': '&quot;',
		"'": '&#039;'
	};
	return text.replace(/[&<>"']/g, (m) => map[m]);
}

/**
 * Detect programming language from code block
 */
function detectLanguage(code: string): string {
	// Simple heuristics for common languages
	if (code.includes('def ') || code.includes('import ')) return 'python';
	if (code.includes('function ') || code.includes('const ') || code.includes('let '))
		return 'javascript';
	if (code.includes('public class') || code.includes('System.out')) return 'java';
	if (code.includes('#include') || code.includes('std::')) return 'cpp';
	if (code.includes('<html') || code.includes('</')) return 'html';
	return 'plaintext';
}

/**
 * Format code for display (no syntax highlighting to avoid escaping issues)
 */
function formatCode(code: string): string {
	return escapeHtml(code);
}

/**
 * Render markdown to HTML
 */
export function renderMarkdown(text: string): string {
	let html = text;

	// Code blocks (```lang\ncode\n```)
	html = html.replace(/```(\w*)\n([\s\S]*?)```/g, (_, lang, code) => {
		const language = lang || detectLanguage(code);
		const formatted = formatCode(code.trim());
		return `<div class="code-block my-4 rounded-lg bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700">
			<div class="flex items-center justify-between px-4 py-2 bg-gray-100 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 rounded-t-lg">
				<span class="text-xs font-mono text-gray-600 dark:text-gray-400 uppercase">${language}</span>
			</div>
			<pre class="p-4 overflow-x-auto"><code class="text-sm font-mono text-gray-800 dark:text-gray-200">${formatted}</code></pre>
		</div>`;
	});

	// Inline code
	html = html.replace(
		/`([^`]+)`/g,
		'<code class="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-sm font-mono text-red-600 dark:text-red-400">$1</code>'
	);

	// Headers
	html = html.replace(/^### (.*$)/gim, '<h3 class="text-lg font-semibold mt-4 mb-2">$1</h3>');
	html = html.replace(/^## (.*$)/gim, '<h2 class="text-xl font-semibold mt-4 mb-2">$1</h2>');
	html = html.replace(/^# (.*$)/gim, '<h1 class="text-2xl font-bold mt-4 mb-2">$1</h1>');

	// Bold
	html = html.replace(/\*\*(.*?)\*\*/g, '<strong class="font-semibold">$1</strong>');

	// Italic
	html = html.replace(/\*(.*?)\*/g, '<em class="italic">$1</em>');

	// Links
	html = html.replace(
		/\[([^\]]+)\]\(([^)]+)\)/g,
		'<a href="$2" class="text-blue-600 dark:text-blue-400 hover:underline" target="_blank" rel="noopener noreferrer">$1</a>'
	);

	// Unordered lists
	html = html.replace(/^\* (.+)$/gim, '<li class="ml-4 list-disc">$1</li>');
	html = html.replace(/(<li class="ml-4 list-disc">.*<\/li>)/s, '<ul class="my-2">$1</ul>');

	// Ordered lists
	html = html.replace(/^\d+\. (.+)$/gim, '<li class="ml-4 list-decimal">$1</li>');
	html = html.replace(/(<li class="ml-4 list-decimal">.*<\/li>)/s, '<ol class="my-2">$1</ol>');

	// Line breaks (double newlines become paragraphs)
	html = html.replace(/\n\n/g, '</p><p class="my-2">');
	html = '<p class="my-2">' + html + '</p>';

	return html;
}

/**
 * Copy text to clipboard
 */
export async function copyToClipboard(text: string): Promise<boolean> {
	try {
		await navigator.clipboard.writeText(text);
		return true;
	} catch (error) {
		console.error('Failed to copy to clipboard:', error);
		return false;
	}
}

/**
 * Extract code from markdown
 */
export function extractCode(markdown: string): string[] {
	const codeBlocks: string[] = [];
	const regex = /```(?:\w*)\n([\s\S]*?)```/g;
	let match;

	while ((match = regex.exec(markdown)) !== null) {
		codeBlocks.push(match[1].trim());
	}

	return codeBlocks;
}
