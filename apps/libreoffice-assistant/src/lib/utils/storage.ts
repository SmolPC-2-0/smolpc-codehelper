function hasLocalStorage(): boolean {
  return typeof window !== 'undefined' && typeof localStorage !== 'undefined';
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
  try {
    if (!hasLocalStorage()) {
      return defaultValue;
    }

    const item = localStorage.getItem(key);
    return item ? (JSON.parse(item) as T) : defaultValue;
  } catch (error) {
    console.error(`Failed to load from localStorage (${key}):`, error);
    return defaultValue;
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
