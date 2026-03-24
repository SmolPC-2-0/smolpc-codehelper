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

/**
 * Format code for display (no syntax highlighting to avoid escaping issues)
 */
function formatCode(code: string): string {
	return escapeHtml(code);
}

type TableAlignment = 'left' | 'center' | 'right';

/**
 * Split a potential markdown table row into cells.
 * Supports optional leading/trailing pipes and requires at least two columns.
 */
function splitTableRow(line: string): string[] | null {
	const trimmed = line.trim();
	if (!trimmed.includes('|')) return null;

	const normalized = trimmed.replace(/^\|/, '').replace(/\|$/, '');
	const cells = normalized.split('|').map((cell) => cell.trim());

	if (cells.length < 2) return null;

	return cells;
}

/**
 * Parse a markdown table delimiter row and extract alignments.
 * Requires at least three dashes per column to avoid misparsing ordinary prose.
 */
function parseTableDelimiter(line: string, expectedColumns: number): TableAlignment[] | null {
	const cells = splitTableRow(line);
	if (!cells || cells.length !== expectedColumns) return null;

	const alignments: TableAlignment[] = [];

	for (const cell of cells) {
		if (!/^:?-{3,}:?$/.test(cell)) return null;

		if (cell.startsWith(':') && cell.endsWith(':')) {
			alignments.push('center');
		} else if (cell.endsWith(':')) {
			alignments.push('right');
		} else {
			alignments.push('left');
		}
	}

	return alignments;
}

/**
 * Generate semantic HTML for a markdown table.
 */
function generateTableHTML(
	headers: string[],
	alignments: TableAlignment[],
	rows: string[][]
): string {
	const renderCellClass = (alignment: TableAlignment) =>
		`markdown-table__cell markdown-table__cell--align-${alignment}`;

	const headerHtml = headers
		.map(
			(header, index) =>
				`<th scope="col" class="${renderCellClass(alignments[index])}">${header}</th>`
		)
		.join('');

	const bodyHtml = rows
		.map(
			(row) =>
				`<tr>${row
					.map((cell, index) => `<td class="${renderCellClass(alignments[index])}">${cell}</td>`)
					.join('')}</tr>`
		)
		.join('');

	return `<div class="markdown-table-wrapper">
		<table class="markdown-table">
			<thead><tr>${headerHtml}</tr></thead>
			<tbody>${bodyHtml}</tbody>
		</table>
	</div>`;
}

/**
 * Preserve line breaks for malformed table-like blocks so they degrade as readable plain text.
 */
function generatePlainPipeBlockHTML(lines: string[]): string {
	const content = lines.join('\n');
	return `<div class="markdown-plain-block">${content}</div>`;
}

/**
 * Replace valid markdown pipe tables with HTML placeholders.
 * Tables require a delimiter row and at least one body row.
 * Invalid table-like blocks remain plain text with preserved line breaks.
 */
function replaceTables(text: string, tableBlocks: string[], plainPipeBlocks: string[]): string {
	const lines = text.split('\n');
	const output: string[] = [];

	for (let i = 0; i < lines.length; i++) {
		const headerCells = splitTableRow(lines[i]);
		const nextRowCells = headerCells ? splitTableRow(lines[i + 1] ?? '') : null;
		const delimiterCells =
			headerCells && headerCells.some((cell) => cell.length > 0)
				? parseTableDelimiter(lines[i + 1] ?? '', headerCells.length)
				: null;

		if (headerCells && nextRowCells && !delimiterCells) {
			const plainLines = [lines[i], lines[i + 1]];
			let rowIndex = i + 2;

			while (rowIndex < lines.length) {
				const rowCells = splitTableRow(lines[rowIndex]);
				if (!rowCells || rowCells.length !== headerCells.length) break;
				plainLines.push(lines[rowIndex]);
				rowIndex++;
			}

			const placeholder = `___PIPEBLOCK${plainPipeBlocks.length}___`;
			plainPipeBlocks.push(generatePlainPipeBlockHTML(plainLines));
			output.push(placeholder);
			i = rowIndex - 1;
			continue;
		}

		if (!headerCells || !delimiterCells) {
			output.push(lines[i]);
			continue;
		}

		const bodyRows: string[][] = [];
		let rowIndex = i + 2;

		while (rowIndex < lines.length) {
			const rowCells = splitTableRow(lines[rowIndex]);
			if (!rowCells || rowCells.length !== headerCells.length) break;
			bodyRows.push(rowCells);
			rowIndex++;
		}

		// Leave malformed or incomplete tables untouched.
		if (bodyRows.length === 0) {
			const placeholder = `___PIPEBLOCK${plainPipeBlocks.length}___`;
			plainPipeBlocks.push(generatePlainPipeBlockHTML([lines[i], lines[i + 1]]));
			output.push(placeholder);
			i = rowIndex - 1;
			continue;
		}

		const placeholder = `___TABLE${tableBlocks.length}___`;
		tableBlocks.push(generateTableHTML(headerCells, delimiterCells, bodyRows));
		output.push(placeholder);
		i = rowIndex - 1;
	}

	return output.join('\n');
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
	return `<div class="code-block code-block-frame">
		<div class="code-block-head">
			<span class="code-block-lang">${language}</span>
			<button
				data-code="${encodedCode}"
				class="code-copy-btn code-copy-btn-frame"
				title="Copy code"
				aria-label="Copy code to clipboard"
			>
				<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"></path>
				</svg>
				<span class="code-copy-btn-label">Copy</span>
			</button>
		</div>
		<pre class="code-block-pre"><code class="code-block-code">${formattedCode}</code></pre>
	</div>`;
}

/**
 * Render markdown to HTML
 */
export function renderMarkdown(text: string): string {
	// Step 1: Extract and process code blocks first (with placeholders)
	const codeBlocks: string[] = [];
	const inlineCodeBlocks: string[] = [];
	const tableBlocks: string[] = [];
	const plainPipeBlocks: string[] = [];

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

	// Step 3: Extract inline code before table parsing so pipes inside code spans do not split cells.
	html = html.replace(/`([^`]+)`/g, (_, code) => {
		const placeholder = `___INLINECODE${inlineCodeBlocks.length}___`;
		inlineCodeBlocks.push(`<code class="inline-code">${code}</code>`);
		return placeholder;
	});

	// Headers (process longest prefix first to avoid ###### matching as #)
	html = html.replace(/^###### (.*$)/gim, '<h6 class="text-sm font-semibold mt-3 mb-1">$1</h6>');
	html = html.replace(/^##### (.*$)/gim, '<h5 class="text-sm font-semibold mt-3 mb-1">$1</h5>');
	html = html.replace(/^#### (.*$)/gim, '<h4 class="text-base font-semibold mt-3 mb-2">$1</h4>');
	html = html.replace(/^### (.*$)/gim, '<h3 class="text-lg font-semibold mt-4 mb-2">$1</h3>');
	html = html.replace(/^## (.*$)/gim, '<h2 class="text-xl font-semibold mt-4 mb-2">$1</h2>');
	html = html.replace(/^# (.*$)/gim, '<h1 class="text-2xl font-bold mt-4 mb-2">$1</h1>');

	// Bold
	html = html.replace(/\*\*(.*?)\*\*/g, '<strong class="font-semibold">$1</strong>');

	// Italic
	html = html.replace(/\*(.*?)\*/g, '<em class="italic">$1</em>');

	// Links (with URL sanitization)
	html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (match, text, url) => {
		const safeUrl = sanitizeUrl(url);
		return `<a href="${safeUrl}" class="markdown-link" target="_blank" rel="noopener noreferrer">${text}</a>`;
	});

	// Tables (require header, delimiter, and at least one body row)
	html = replaceTables(html, tableBlocks, plainPipeBlocks);

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
	// Headings (h1-h6)
	html = html.replace(/<p class="my-2">(<h[1-6])/g, '$1');
	html = html.replace(/(<\/h[1-6]>)<\/p>/g, '$1');
	// Lists
	html = html.replace(/<p class="my-2">(<ul)/g, '$1');
	html = html.replace(/(<\/ul>)<\/p>/g, '$1');
	html = html.replace(/<p class="my-2">(<ol)/g, '$1');
	html = html.replace(/(<\/ol>)<\/p>/g, '$1');
	// Code blocks
	html = html.replace(/<p class="my-2">(___CODEBLOCK\d+___)/g, '$1');
	html = html.replace(/(___CODEBLOCK\d+___)<\/p>/g, '$1');
	// Tables
	html = html.replace(/<p class="my-2">(___TABLE\d+___)/g, '$1');
	html = html.replace(/(___TABLE\d+___)<\/p>/g, '$1');
	// Plain pipe blocks
	html = html.replace(/<p class="my-2">(___PIPEBLOCK\d+___)/g, '$1');
	html = html.replace(/(___PIPEBLOCK\d+___)<\/p>/g, '$1');

	// Final step: Restore code blocks from placeholders
	codeBlocks.forEach((block, i) => {
		html = html.replace(`___CODEBLOCK${i}___`, block);
	});

	tableBlocks.forEach((block, i) => {
		html = html.replace(`___TABLE${i}___`, block);
	});

	plainPipeBlocks.forEach((block, i) => {
		html = html.replace(`___PIPEBLOCK${i}___`, block);
	});

	inlineCodeBlocks.forEach((block, i) => {
		html = html.replace(`___INLINECODE${i}___`, block);
	});

	// Clean up any remaining paragraph tags around code blocks
	html = html.replace(/<p class="my-2">(<div class="code-block)/g, '$1');
	html = html.replace(/<p class="my-2">(<div class="markdown-table-wrapper)/g, '$1');
	html = html.replace(/<p class="my-2">(<div class="markdown-plain-block)/g, '$1');
	html = html.replace(/(<\/div>)<\/p>/g, '$1');

	// Sanitize with DOMPurify to prevent XSS attacks
	// This is the final security layer that catches any malicious HTML
	return DOMPurify.sanitize(html, {
		ALLOWED_TAGS: [
			'h1',
			'h2',
			'h3',
			'h4',
			'h5',
			'h6',
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
			'table',
			'thead',
			'tbody',
			'tr',
			'th',
			'td',
			'svg',
			'path'
		],
		ALLOWED_ATTR: [
			'href',
			'class',
			'scope',
			'data-code',
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
			button.innerHTML = `<span class="code-copy-btn-label code-copy-btn-label--success">Copied</span>`;

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
