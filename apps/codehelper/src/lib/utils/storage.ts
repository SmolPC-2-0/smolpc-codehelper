/**
 * Save data to localStorage with type safety
 */
export function saveToStorage<T>(key: string, data: T): void {
	try {
		localStorage.setItem(key, JSON.stringify(data));
	} catch (error) {
		console.error(`Failed to save to localStorage (${key}):`, error);
	}
}

/**
 * Load data from localStorage with type safety and default value
 */
export function loadFromStorage<T>(key: string, defaultValue: T): T {
	try {
		// Check if localStorage is available (might not be in some contexts)
		if (typeof window === 'undefined' || typeof localStorage === 'undefined') {
			console.warn('localStorage not available, using default value');
			return defaultValue;
		}

		const item = localStorage.getItem(key);
		if (item === null) {
			return defaultValue;
		}
		return JSON.parse(item) as T;
	} catch (error) {
		console.error(`Failed to load from localStorage (${key}):`, error);
		return defaultValue;
	}
}

/**
 * Remove item from localStorage
 */
export function removeFromStorage(key: string): void {
	try {
		localStorage.removeItem(key);
	} catch (error) {
		console.error(`Failed to remove from localStorage (${key}):`, error);
	}
}

/**
 * Clear all localStorage
 */
export function clearStorage(): void {
	try {
		localStorage.clear();
	} catch (error) {
		console.error('Failed to clear localStorage:', error);
	}
}
