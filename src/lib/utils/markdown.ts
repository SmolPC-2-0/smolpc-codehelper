/**
 * Custom lightweight markdown renderer with DOMPurify sanitization
 *
 * Security: All HTML output is sanitized with DOMPurify to prevent XSS attacks.
 * This includes protection against:
 * - Script injection
 * - Unsafe URL protocols (javascript:, data:, etc.)
 * - Malicious HTML attributes
 * - SVG-based attacks
 *
 * NOTE: For production use, consider migrating to 'marked' package:
 * npm install marked
 *
 * Then replace this implementation with:
 * import { marked } from 'marked';
 * export function renderMarkdown(text: string): string {
 *   return DOMPurify.sanitize(marked(text));
 * }
 */

import DOMPurify from 'isomorphic-dompurify';

export interface ParsedCodeBlock {
	index: number;
	language: string;
	code: string;
	fileName: string;
}

interface RenderMarkdownOptions {
	showGenerateButtons?: boolean;
}

interface CodeActionHandlers {
	onGenerateFile?: (blockIndex: number) => void | Promise<void>;
}

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
 * Validate URL to prevent XSS attacks via unsafe protocols
 * Only allows http, https, and mailto protocols
 */
function sanitizeUrl(url: string): string {
	const allowedProtocols = /^(?:https?|mailto):/i;

	// Trim whitespace
	url = url.trim();

	// Check for allowed protocols
	if (!allowedProtocols.test(url)) {
		// If it starts with a protocol we don't allow, return safe placeholder
		if (/^[a-z][a-z0-9+.-]*:/i.test(url)) {
			return '#';
		}
		// If it's a relative URL or fragment, allow it
		if (url.startsWith('/') || url.startsWith('#') || url.startsWith('.')) {
			return url;
		}
		// Default to https for bare URLs
		return 'https://' + url;
	}

	return url;
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

function normalizeLanguage(language: string): string {
	const normalized = language.trim().toLowerCase();
	if (normalized === 'js') return 'javascript';
	if (normalized === 'ts') return 'typescript';
	if (normalized === 'py') return 'python';
	if (normalized === 'rs') return 'rust';
	if (normalized === 'sh') return 'bash';
	if (!normalized) return 'plaintext';
	return normalized;
}

function looksLikeFileName(value: string): boolean {
	const trimmed = value.trim();
	if (!trimmed) return false;
	if (trimmed.includes('/') || trimmed.includes('\\')) return true;
	return /\.[A-Za-z0-9_-]{1,10}$/.test(trimmed);
}

function sanitizeFileNameHint(fileName: string): string {
	const withoutQuotes = fileName.trim().replace(/^["'`]+|["'`]+$/g, '');
	const basename = withoutQuotes.split(/[\\/]/).pop() ?? withoutQuotes;
	return basename.trim();
}

function parseCodeFenceInfo(info: string): { languageHint: string; fileNameHint: string | null } {
	const parts = info.trim().split(/\s+/).filter(Boolean);
	if (parts.length === 0) {
		return { languageHint: '', fileNameHint: null };
	}

	if (parts.length === 1) {
		if (looksLikeFileName(parts[0])) {
			return { languageHint: '', fileNameHint: sanitizeFileNameHint(parts[0]) };
		}
		return { languageHint: parts[0], fileNameHint: null };
	}

	if (looksLikeFileName(parts[1])) {
		return {
			languageHint: parts[0],
			fileNameHint: sanitizeFileNameHint(parts[1])
		};
	}

	if (looksLikeFileName(parts[0])) {
		return {
			languageHint: parts[1] ?? '',
			fileNameHint: sanitizeFileNameHint(parts[0])
		};
	}

	return { languageHint: parts[0], fileNameHint: null };
}

function splitFileName(fileName: string): { stem: string; ext: string } {
	const index = fileName.lastIndexOf('.');
	if (index <= 0 || index === fileName.length - 1) {
		return { stem: fileName, ext: '' };
	}
	return {
		stem: fileName.slice(0, index),
		ext: fileName.slice(index)
	};
}

function inferFileName(language: string): string {
	switch (normalizeLanguage(language)) {
		case 'html':
			return 'index.html';
		case 'css':
			return 'styles.css';
		case 'javascript':
			return 'script.js';
		case 'typescript':
			return 'script.ts';
		case 'json':
			return 'data.json';
		case 'python':
			return 'main.py';
		case 'rust':
			return 'main.rs';
		case 'bash':
			return 'script.sh';
		default:
			return 'file.txt';
	}
}

function dedupeFileNames(fileNames: string[]): string[] {
	const used = new Set<string>();
	return fileNames.map((original) => {
		const baseName = original.trim() || 'file.txt';
		const lower = baseName.toLowerCase();
		if (!used.has(lower)) {
			used.add(lower);
			return baseName;
		}

		const { stem, ext } = splitFileName(baseName);
		let counter = 2;
		while (true) {
			const candidate = `${stem}-${counter}${ext}`;
			const candidateLower = candidate.toLowerCase();
			if (!used.has(candidateLower)) {
				used.add(candidateLower);
				return candidate;
			}
			counter += 1;
		}
	});
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
 * Generate HTML for a code block with action buttons.
 */
function generateCodeBlockHTML(
	language: string,
	fileName: string,
	formattedCode: string,
	encodedCode: string,
	showGenerateButton: boolean,
	blockIndex: number
): string {
	const generateButton = showGenerateButton
		? `<button
				data-block-index="${blockIndex}"
				class="code-generate-btn code-copy-btn-frame"
				title="Generate file in workspace"
				aria-label="Generate file in workspace"
			>
				Generate File
			</button>`
		: '';

	return `<div class="code-block code-block-frame">
		<div class="code-block-head">
			<div class="code-block-meta">
				<span class="code-block-lang">${language}</span>
				<span class="code-block-file">${escapeHtml(fileName)}</span>
			</div>
			<div class="code-block-actions">
				${generateButton}
				<button
					data-code="${encodedCode}"
					class="code-copy-btn code-copy-btn-frame"
					title="Copy code"
					aria-label="Copy code to clipboard"
				>
					<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"></path>
					</svg>
				</button>
			</div>
		</div>
		<pre class="code-block-pre"><code class="code-block-code">${formattedCode}</code></pre>
	</div>`;
}

/**
 * Extract structured code blocks from markdown.
 */
export function extractCodeBlocks(markdown: string): ParsedCodeBlock[] {
	const rawBlocks: Array<{ language: string; code: string; fileName: string }> = [];
	const regex = /```([^\n]*)\n([\s\S]*?)```/g;
	let match: RegExpExecArray | null;

	while ((match = regex.exec(markdown)) !== null) {
		const info = match[1] ?? '';
		const rawCode = match[2] ?? '';
		const code = rawCode.trim();
		const parsedInfo = parseCodeFenceInfo(info);
		const language = normalizeLanguage(parsedInfo.languageHint || detectLanguage(code));
		const hintedName = parsedInfo.fileNameHint ? sanitizeFileNameHint(parsedInfo.fileNameHint) : '';
		const fileName = hintedName || inferFileName(language);
		rawBlocks.push({
			language,
			code,
			fileName
		});
	}

	const dedupedNames = dedupeFileNames(rawBlocks.map((block) => block.fileName));
	return rawBlocks.map((block, index) => ({
		index,
		language: block.language,
		code: block.code,
		fileName: dedupedNames[index]
	}));
}

/**
 * Render markdown to HTML
 */
export function renderMarkdown(text: string, options: RenderMarkdownOptions = {}): string {
	const { showGenerateButtons = false } = options;
	const parsedCodeBlocks = extractCodeBlocks(text);

	// Step 1: Extract and process code blocks first (with placeholders)
	const codeBlocks: string[] = [];
	let parsedIndex = 0;

	// Handle complete code blocks (with closing backticks)
	let html = text.replace(/```([^\n]*)\n([\s\S]*?)```/g, (_, __, code) => {
		const parsed = parsedCodeBlocks[parsedIndex];
		const rawCode = String(code ?? '').trim();
		const language = parsed?.language ?? normalizeLanguage(detectLanguage(rawCode));
		const fileName = parsed?.fileName ?? inferFileName(language);
		const formatted = formatCode(rawCode);
		const encodedCode = encodeBase64(rawCode);
		const blockIndex = parsedIndex;
		const placeholder = `___CODEBLOCK${codeBlocks.length}___`;
		codeBlocks.push(
			generateCodeBlockHTML(
				language,
				fileName,
				formatted,
				encodedCode,
				showGenerateButtons,
				blockIndex
			)
		);
		parsedIndex += 1;
		return placeholder;
	});

	// Handle incomplete/unclosed code blocks (for streaming support)
	html = html.replace(/```([^\n]*)\n([\s\S]*)$/g, (_, lang, code) => {
		const rawCode = String(code ?? '').trim();
		const language = normalizeLanguage(String(lang ?? '').trim() || detectLanguage(rawCode));
		const fileName = inferFileName(language);
		const formatted = formatCode(rawCode);
		const encodedCode = encodeBase64(rawCode);
		const placeholder = `___CODEBLOCK${codeBlocks.length}___`;
		codeBlocks.push(
			generateCodeBlockHTML(language, fileName, formatted, encodedCode, false, -1)
		);
		return placeholder;
	});

	// Step 2: Escape HTML in the remaining text (protects against XSS and preserves angle brackets)
	html = escapeHtml(html);

	// Step 3: Process inline code (backticks survived HTML escaping, content is already escaped)
	html = html.replace(
		/`([^`]+)`/g,
		'<code class="inline-code">$1</code>'
	);

	// Headers
	html = html.replace(/^### (.*$)/gim, '<h3 class="text-lg font-semibold mt-4 mb-2">$1</h3>');
	html = html.replace(/^## (.*$)/gim, '<h2 class="text-xl font-semibold mt-4 mb-2">$1</h2>');
	html = html.replace(/^# (.*$)/gim, '<h1 class="text-2xl font-bold mt-4 mb-2">$1</h1>');

	// Bold
	html = html.replace(/\*\*(.*?)\*\*/g, '<strong class="font-semibold">$1</strong>');

	// Italic
	html = html.replace(/\*(.*?)\*/g, '<em class="italic">$1</em>');

	// Links (with URL sanitization)
	html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (match, linkText, url) => {
		const safeUrl = sanitizeUrl(url);
		return `<a href="${safeUrl}" class="markdown-link" target="_blank" rel="noopener noreferrer">${linkText}</a>`;
	});

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

	// Sanitize with DOMPurify to prevent XSS attacks
	// This is the final security layer that catches any malicious HTML
	return DOMPurify.sanitize(html, {
		ALLOWED_TAGS: [
			'h1',
			'h2',
			'h3',
			'p',
			'br',
			'strong',
			'em',
			'code',
			'pre',
			'ul',
			'ol',
			'li',
			'a',
			'div',
			'span',
			'button',
			'svg',
			'path'
		],
		ALLOWED_ATTR: [
			'href',
			'class',
			'data-code',
			'data-block-index',
			'title',
			'aria-label',
			'target',
			'rel',
			'stroke',
			'fill',
			'viewBox',
			'stroke-linecap',
			'stroke-linejoin',
			'stroke-width',
			'd'
		],
		ALLOWED_URI_REGEXP: /^(?:https?|mailto|#):/i,
		KEEP_CONTENT: true,
		RETURN_DOM_FRAGMENT: false,
		RETURN_DOM: false
	});
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
	return extractCodeBlocks(markdown).map((block) => block.code);
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
 * Setup event delegation for code action buttons.
 * Call this from your component's onMount to avoid inline onclick handlers.
 */
export function setupCodeActionHandlers(
	container: HTMLElement,
	handlers: CodeActionHandlers = {}
): () => void {
	const handleClick = async (event: Event) => {
		const target = event.target as HTMLElement;
		const copyButton = target.closest('.code-copy-btn') as HTMLButtonElement | null;
		if (copyButton) {
			const encodedCode = copyButton.dataset.code;
			if (!encodedCode) return;

			try {
				const code = decodeBase64(encodedCode);
				await navigator.clipboard.writeText(code);

				// Show success feedback
				const originalHTML = copyButton.innerHTML;
				copyButton.innerHTML = `<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
				</svg>`;

				setTimeout(() => {
					copyButton.innerHTML = originalHTML;
				}, 2000);
			} catch (error) {
				console.error('Failed to copy code:', error);
			}
			return;
		}

		const generateButton = target.closest('.code-generate-btn') as HTMLButtonElement | null;
		if (!generateButton || !handlers.onGenerateFile) {
			return;
		}

		const blockIndexRaw = generateButton.dataset.blockIndex;
		const blockIndex = Number(blockIndexRaw);
		if (!Number.isInteger(blockIndex) || blockIndex < 0) {
			return;
		}

		try {
			await handlers.onGenerateFile(blockIndex);
		} catch (error) {
			console.error('Failed to generate file:', error);
		}
	};

	container.addEventListener('click', handleClick);

	// Return cleanup function
	return () => {
		container.removeEventListener('click', handleClick);
	};
}

/**
 * Backwards-compatible alias for legacy callers.
 */
export function setupCodeCopyHandlers(container: HTMLElement): () => void {
	return setupCodeActionHandlers(container);
}
