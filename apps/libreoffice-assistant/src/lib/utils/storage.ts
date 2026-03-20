function hasLocalStorage(): boolean {
  return typeof window !== 'undefined' && typeof localStorage !== 'undefined';
}

export type StorageLoadStatus = 'ok' | 'missing' | 'unavailable' | 'parse_error';

export interface StorageLoadResult<T> {
  status: StorageLoadStatus;
  value?: T;
}

export function saveToStorage<T>(key: string, data: T): boolean {
  try {
    if (!hasLocalStorage()) {
      return false;
    }

    localStorage.setItem(key, JSON.stringify(data));
    return true;
  } catch (error) {
    console.error(`Failed to save to localStorage (${key}):`, error);
    return false;
  }
}

export function loadFromStorage<T>(key: string, defaultValue: T): T {
  const result = loadFromStorageWithStatus<T>(key);
  return result.status === 'ok' ? (result.value as T) : defaultValue;
}

export function loadFromStorageWithStatus<T>(key: string): StorageLoadResult<T> {
  try {
    if (!hasLocalStorage()) {
      return { status: 'unavailable' };
    }

    const item = localStorage.getItem(key);
    if (item === null) {
      return { status: 'missing' };
    }

    return {
      status: 'ok',
      value: JSON.parse(item) as T
    };
  } catch (error) {
    console.error(`Failed to load from localStorage (${key}):`, error);
    return { status: 'parse_error' };
  }
}

export function removeFromStorage(key: string): boolean {
  try {
    if (!hasLocalStorage()) {
      return false;
    }

    localStorage.removeItem(key);
    return true;
  } catch (error) {
    console.error(`Failed to remove localStorage key (${key}):`, error);
    return false;
  }
}
