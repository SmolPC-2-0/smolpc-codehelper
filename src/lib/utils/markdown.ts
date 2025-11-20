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
 * Modern Base64 encoding with proper Unicode support
 * Replaces deprecated unescape/escape functions
 */
function encodeBase64(str: string): string {
	// Convert string to UTF-8 bytes, then to base64
	const bytes = new TextEncoder().encode(str);
	const binString = Array.from(bytes, (byte) => String.fromCodePoint(byte)).join('');
	return btoa(binString);
}

/**
 * Generate HTML for a code block with copy button
 * Extracted to avoid duplication (DRY principle)
 */
function generateCodeBlockHTML(
	language: string,
	formattedCode: string,
	encodedCode: string
): string {
	return `<div class="code-block my-4 rounded-lg bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700">
		<div class="flex items-center justify-between px-4 py-2 bg-gray-100 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 rounded-t-lg">
			<span class="text-xs font-mono text-gray-600 dark:text-gray-400 uppercase">${language}</span>
			<button
				data-code="${encodedCode}"
				class="code-copy-btn p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-600 dark:text-gray-400 transition-colors"
				title="Copy code"
				aria-label="Copy code to clipboard"
			>
				<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"></path>
				</svg>
			</button>
		</div>
		<pre class="p-4 overflow-x-auto"><code class="text-sm font-mono text-gray-800 dark:text-gray-200">${formattedCode}</code></pre>
	</div>`;
}

/**
 * Render markdown to HTML
 */
export function renderMarkdown(text: string): string {
	// Step 1: Extract and process code blocks first (with placeholders)
	const codeBlocks: string[] = [];

	// Handle complete code blocks (with closing backticks)
	let html = text.replace(/```(\w*)\n([\s\S]*?)```/g, (_, lang, code) => {
		const language = lang || detectLanguage(code);
		const formatted = formatCode(code.trim());
		const rawCode = code.trim();
		const encodedCode = encodeBase64(rawCode);
		const placeholder = `___CODEBLOCK${codeBlocks.length}___`;
		codeBlocks.push(generateCodeBlockHTML(language, formatted, encodedCode));
		return placeholder;
	});

	// Handle incomplete/unclosed code blocks (for streaming support)
	html = html.replace(/```(\w*)\n([\s\S]*)$/g, (_, lang, code) => {
		const language = lang || detectLanguage(code);
		const formatted = formatCode(code.trim());
		const rawCode = code.trim();
		const encodedCode = encodeBase64(rawCode);
		const placeholder = `___CODEBLOCK${codeBlocks.length}___`;
		codeBlocks.push(generateCodeBlockHTML(language, formatted, encodedCode));
		return placeholder;
	});

	// Step 2: Escape HTML in the remaining text (protects against XSS and preserves angle brackets)
	html = escapeHtml(html);

	// Step 3: Process inline code (backticks survived HTML escaping, content is already escaped)
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

	// Unordered lists (support both * and - markers)
	html = html.replace(/^[*-] (.+)$/gim, '<li class="ml-6 list-disc">$1</li>');
	html = html.replace(/(<li class="ml-6 list-disc">.*<\/li>)/s, '<ul class="my-2 pl-2">$1</ul>');

	// Ordered lists
	html = html.replace(/^\d+\. (.+)$/gim, '<li class="ml-6 list-decimal">$1</li>');
	html = html.replace(/(<li class="ml-6 list-decimal">.*<\/li>)/s, '<ol class="my-2 pl-2">$1</ol>');

	// Line breaks (double newlines become paragraphs)
	html = html.replace(/\n\n/g, '</p><p class="my-2">');
	html = '<p class="my-2">' + html + '</p>';

	// Fix invalid HTML: remove paragraph tags around block-level elements
	// Headings
	html = html.replace(/<p class="my-2">(<h[123])/g, '$1');
	html = html.replace(/(<\/h[123]>)<\/p>/g, '$1');
	// Lists
	html = html.replace(/<p class="my-2">(<ul)/g, '$1');
	html = html.replace(/(<\/ul>)<\/p>/g, '$1');
	html = html.replace(/<p class="my-2">(<ol)/g, '$1');
	html = html.replace(/(<\/ol>)<\/p>/g, '$1');
	// Code blocks
	html = html.replace(/<p class="my-2">(___CODEBLOCK\d+___)/g, '$1');
	html = html.replace(/(___CODEBLOCK\d+___)<\/p>/g, '$1');

	// Final step: Restore code blocks from placeholders
	codeBlocks.forEach((block, i) => {
		html = html.replace(`___CODEBLOCK${i}___`, block);
	});

	// Clean up any remaining paragraph tags around code blocks
	html = html.replace(/<p class="my-2">(<div class="code-block)/g, '$1');
	html = html.replace(/(<\/div>)<\/p>/g, '$1');

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

/**
 * Modern Base64 decoding with proper Unicode support
 * Replaces deprecated unescape/escape functions
 */
function decodeBase64(base64: string): string {
	const binString = atob(base64);
	const bytes = Uint8Array.from(binString, (char) => char.codePointAt(0)!);
	return new TextDecoder().decode(bytes);
}

/**
 * Setup event delegation for code copy buttons
 * Call this from your component's onMount to avoid inline onclick handlers
 * This is CSP-compliant and follows best practices
 */
export function setupCodeCopyHandlers(container: HTMLElement): () => void {
	const handleClick = async (event: Event) => {
		const target = event.target as HTMLElement;
		const button = target.closest('.code-copy-btn') as HTMLButtonElement;

		if (!button) return;

		const encodedCode = button.dataset.code;
		if (!encodedCode) return;

		try {
			const code = decodeBase64(encodedCode);
			await navigator.clipboard.writeText(code);

			// Show success feedback
			const originalHTML = button.innerHTML;
			button.innerHTML = `<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
			</svg>`;

			setTimeout(() => {
				button.innerHTML = originalHTML;
			}, 2000);
		} catch (error) {
			console.error('Failed to copy code:', error);
		}
	};

	container.addEventListener('click', handleClick);

	// Return cleanup function
	return () => {
		container.removeEventListener('click', handleClick);
	};
}
