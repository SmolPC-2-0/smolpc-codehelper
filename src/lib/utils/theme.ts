import type { AppSettings } from '$lib/types/settings';

export type ThemeMode = AppSettings['theme'];
export type ResolvedTheme = 'light' | 'dark';

const THEME_QUERY = '(prefers-color-scheme: dark)';
const DARK_CLASS = 'dark';
const MEDIA_EVENT = 'change';

function getSystemPreference(): ResolvedTheme {
	if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
		return 'light';
	}
	return window.matchMedia(THEME_QUERY).matches ? 'dark' : 'light';
}

export function resolveTheme(theme: ThemeMode): ResolvedTheme {
	if (theme === 'system') {
		return getSystemPreference();
	}
	return theme;
}

export function applyTheme(theme: ThemeMode): ResolvedTheme {
	const resolved = resolveTheme(theme);
	if (typeof document === 'undefined') {
		return resolved;
	}

	const root = document.documentElement;
	root.classList.toggle(DARK_CLASS, resolved === 'dark');
	root.dataset.theme = theme;
	root.dataset.resolvedTheme = resolved;

	return resolved;
}

export function watchSystemTheme(onChange: () => void): () => void {
	if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
		return () => {};
	}

	const mediaQuery = window.matchMedia(THEME_QUERY);
	const handler = () => onChange();

	if (typeof mediaQuery.addEventListener === 'function') {
		mediaQuery.addEventListener(MEDIA_EVENT, handler);
		return () => mediaQuery.removeEventListener(MEDIA_EVENT, handler);
	}

	mediaQuery.addListener(handler);
	return () => mediaQuery.removeListener(handler);
}
