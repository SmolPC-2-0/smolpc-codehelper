export type ActiveOverlay = 'none' | 'hardware' | 'benchmark';

export interface UiState {
	isSidebarOpen: boolean;
	showQuickExamples: boolean;
	activeOverlay: ActiveOverlay;
	userHasScrolledUp: boolean;
}

const initialState: UiState = {
	isSidebarOpen: true,
	showQuickExamples: true,
	activeOverlay: 'none',
	userHasScrolledUp: false
};

let state = $state<UiState>({ ...initialState });

export const uiStore = {
	get state() {
		return state;
	},
	get isSidebarOpen() {
		return state.isSidebarOpen;
	},
	get showQuickExamples() {
		return state.showQuickExamples;
	},
	get activeOverlay() {
		return state.activeOverlay;
	},
	get userHasScrolledUp() {
		return state.userHasScrolledUp;
	},

	setSidebarOpen(isOpen: boolean) {
		state.isSidebarOpen = isOpen;
	},

	toggleSidebar() {
		state.isSidebarOpen = !state.isSidebarOpen;
	},

	setShowQuickExamples(show: boolean) {
		state.showQuickExamples = show;
	},

	openOverlay(overlay: Exclude<ActiveOverlay, 'none'>) {
		state.activeOverlay = overlay;
	},

	toggleOverlay(overlay: Exclude<ActiveOverlay, 'none'>) {
		state.activeOverlay = state.activeOverlay === overlay ? 'none' : overlay;
	},

	closeOverlay() {
		state.activeOverlay = 'none';
	},

	setUserHasScrolledUp(scrolledUp: boolean) {
		state.userHasScrolledUp = scrolledUp;
	},

	resetScrollState() {
		state.userHasScrolledUp = false;
	},

	reset() {
		state = { ...initialState };
	}
};
