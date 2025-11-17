// Benchmark store for tracking benchmark execution state

export interface BenchmarkProgress {
	current: number;
	total: number;
	current_test: string;
	iteration: number;
}

export interface BenchmarkState {
	isRunning: boolean;
	progress: BenchmarkProgress | null;
	error: string | null;
	lastResultPath: string | null;
}

const initialState: BenchmarkState = {
	isRunning: false,
	progress: null,
	error: null,
	lastResultPath: null
};

// Svelte 5 state using runes
let state = $state<BenchmarkState>({ ...initialState });

// Store object with methods
export const benchmarkStore = {
	// Getters
	get state() {
		return state;
	},
	get isRunning() {
		return state.isRunning;
	},
	get progress() {
		return state.progress;
	},
	get error() {
		return state.error;
	},
	get lastResultPath() {
		return state.lastResultPath;
	},

	// Actions
	start() {
		state.isRunning = true;
		state.progress = null;
		state.error = null;
		state.lastResultPath = null;
	},

	updateProgress(progress: BenchmarkProgress) {
		state.progress = progress;
	},

	complete(resultPath: string) {
		state.isRunning = false;
		state.lastResultPath = resultPath;
		state.progress = null;
	},

	setError(error: string) {
		state.isRunning = false;
		state.error = error;
		state.progress = null;
	},

	reset() {
		state = { ...initialState };
	}
};
