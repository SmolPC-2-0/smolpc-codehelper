export type SetupItemState = 'ready' | 'missing' | 'not_prepared' | 'error';
export type SetupOverallState = 'ready' | 'needs_attention' | 'error';

export interface SetupItemDto {
	id: string;
	label: string;
	state: SetupItemState;
	detail: string | null;
	required: boolean;
	canPrepare: boolean;
}

export interface SetupStatusDto {
	overallState: SetupOverallState;
	items: SetupItemDto[];
	lastError: string | null;
}
