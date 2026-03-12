export interface LauncherAppSummary {
	app_id: string;
	display_name: string;
	exe_path: string;
	icon: string | null;
	is_running: boolean;
	has_focus_command: boolean;
	min_engine_api_major: number | null;
}

export interface EngineStatusSummary {
	reachable: boolean;
	ready: boolean;
	state: string | null;
	active_model: string | null;
}

export interface EngineApiGateInfo {
	required_major: number | null;
	actual_version: string;
	actual_major: number | null;
	source: string;
}

export interface LauncherLaunchResult {
	app_id: string;
	action: 'launched' | 'focused';
	readiness_state: string | null;
	readiness_attempt_id: string | null;
	engine_api_gate: EngineApiGateInfo;
}
