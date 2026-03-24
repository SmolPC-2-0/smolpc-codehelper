const STORAGE_ENVELOPE_MARKER = '__smolpcStorageEnvelopeV1';
const PRIMARY_KEY_SUFFIX = '__primary_v1';
const BACKUP_KEY_SUFFIX = '__backup_v1';

interface StorageEnvelope {
	__smolpcStorageEnvelopeV1: true;
	checksum: string;
	writtenAt: number;
	payload: unknown;
}

function isStorageAvailable(): boolean {
	return typeof window !== 'undefined' && typeof localStorage !== 'undefined';
}

function primaryStorageKey(key: string): string {
	return `${key}${PRIMARY_KEY_SUFFIX}`;
}

function backupStorageKey(key: string): string {
	return `${key}${BACKUP_KEY_SUFFIX}`;
}

function checksum(value: string): string {
	// FNV-1a 32-bit hash. Good enough for corruption detection.
	let hash = 0x811c9dc5;
	for (let index = 0; index < value.length; index += 1) {
		hash ^= value.charCodeAt(index);
		hash = Math.imul(hash, 0x01000193);
	}
	return (hash >>> 0).toString(16).padStart(8, '0');
}

function isStorageEnvelope(value: unknown): value is StorageEnvelope {
	if (!value || typeof value !== 'object') {
		return false;
	}

	const candidate = value as Record<string, unknown>;
	return candidate[STORAGE_ENVELOPE_MARKER] === true;
}

function tryParseValue<T>(raw: string): { ok: true; value: T } | { ok: false } {
	try {
		const parsed = JSON.parse(raw) as unknown;
		if (!isStorageEnvelope(parsed)) {
			return { ok: true, value: parsed as T };
		}

		const payloadRaw = JSON.stringify(parsed.payload);
		if (checksum(payloadRaw) !== parsed.checksum) {
			return { ok: false };
		}

		return { ok: true, value: parsed.payload as T };
	} catch {
		return { ok: false };
	}
}

function serializeEnvelope<T>(data: T): string {
	const payloadRaw = JSON.stringify(data);
	const envelope: StorageEnvelope = {
		[STORAGE_ENVELOPE_MARKER]: true,
		checksum: checksum(payloadRaw),
		writtenAt: Date.now(),
		payload: data
	};
	return JSON.stringify(envelope);
}

/**
 * Save data to localStorage with corruption fallback safety.
 */
export function saveToStorage<T>(key: string, data: T): void {
	try {
		if (!isStorageAvailable()) {
			return;
		}

		const primaryKey = primaryStorageKey(key);
		const backupKey = backupStorageKey(key);
		const serialized = serializeEnvelope(data);
		const currentPrimary = localStorage.getItem(primaryKey) ?? localStorage.getItem(key);

		if (currentPrimary && currentPrimary !== serialized) {
			localStorage.setItem(backupKey, currentPrimary);
		}

		localStorage.setItem(primaryKey, serialized);
	} catch (error) {
		console.error(`Failed to save to localStorage (${key}):`, error);
	}
}

/**
 * Load data from localStorage with fallback to backup slot and legacy key migration.
 */
export function loadFromStorage<T>(key: string, defaultValue: T): T {
	try {
		if (!isStorageAvailable()) {
			console.warn('localStorage not available, using default value');
			return defaultValue;
		}

		const primaryKey = primaryStorageKey(key);
		const backupKey = backupStorageKey(key);
		const readOrder: Array<{ storageKey: string; migrateLegacy?: boolean }> = [
			{ storageKey: primaryKey },
			{ storageKey: backupKey },
			{ storageKey: key, migrateLegacy: true }
		];

		for (const { storageKey, migrateLegacy } of readOrder) {
			const raw = localStorage.getItem(storageKey);
			if (raw === null) {
				continue;
			}

			const parsed = tryParseValue<T>(raw);
			if (!parsed.ok) {
				continue;
			}

			if (migrateLegacy) {
				saveToStorage(key, parsed.value);
				localStorage.removeItem(key);
			}

			return parsed.value;
		}

		return defaultValue;
	} catch (error) {
		console.error(`Failed to load from localStorage (${key}):`, error);
		return defaultValue;
	}
}

/**
 * Remove item from localStorage (primary, backup, and legacy slot).
 */
export function removeFromStorage(key: string): void {
	try {
		if (!isStorageAvailable()) {
			return;
		}

		localStorage.removeItem(primaryStorageKey(key));
		localStorage.removeItem(backupStorageKey(key));
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
		if (!isStorageAvailable()) {
			return;
		}

		localStorage.clear();
	} catch (error) {
		console.error('Failed to clear localStorage:', error);
	}
}
