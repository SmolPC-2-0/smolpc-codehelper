import { loadFromStorage, saveToStorage } from '$lib/utils/storage';

const STORAGE_KEY = 'smolpc_composer_drafts_v1';

type DraftsByKey = Record<string, string>;

const initialDrafts = loadFromStorage<DraftsByKey>(STORAGE_KEY, {});

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
