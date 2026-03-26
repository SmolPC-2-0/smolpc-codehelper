import { loadFromStorage, saveToStorage } from '$lib/utils/storage';

const STORAGE_KEY = 'smolpc_composer_drafts_v1';

type DraftsByKey = Record<string, string>;

const storedDrafts = loadFromStorage<DraftsByKey>(STORAGE_KEY, {});
const initialDrafts = Object.fromEntries(
	Object.entries(storedDrafts).filter(([key]) => !key.startsWith('calc:'))
);

if (Object.keys(initialDrafts).length !== Object.keys(storedDrafts).length) {
	saveToStorage(STORAGE_KEY, initialDrafts);
}

let draftsByKey = $state<DraftsByKey>(initialDrafts);

function persist(): void {
	saveToStorage(STORAGE_KEY, draftsByKey);
}

export const composerDraftStore = {
	getDraft(key: string): string {
		return draftsByKey[key] ?? '';
	},

	setDraft(key: string, value: string): void {
		if (!value) {
			if (!(key in draftsByKey)) {
				return;
			}
			const remaining = { ...draftsByKey };
			delete remaining[key];
			draftsByKey = remaining;
			persist();
			return;
		}

		if (draftsByKey[key] === value) {
			return;
		}

		draftsByKey = {
			...draftsByKey,
			[key]: value
		};
		persist();
	},

	clearDraft(key: string): void {
		if (!(key in draftsByKey)) {
			return;
		}

		const remaining = { ...draftsByKey };
		delete remaining[key];
		draftsByKey = remaining;
		persist();
	}
};
