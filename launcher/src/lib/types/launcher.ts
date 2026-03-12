export interface LauncherAppSummary {
	app_id: string;
	display_name: string;
	icon: string | null;
	min_engine_api_major: number | null;
	install_state: LauncherInstallState;
	exe_path: string | null;
	is_running: boolean;
	has_launch_command: boolean;
	has_focus_command: boolean;
	can_install: boolean;
	can_repair: boolean;
	manual_registration_required: boolean;
}

export type LauncherInstallState = 'not_installed' | 'installed' | 'broken';

export type LauncherInstallOutcome = 'installed' | 'retry_required' | 'manual_required';

export interface LauncherInstallResult {
	app_id: string;
	outcome: LauncherInstallOutcome;
	message: string;
	exe_path: string | null;
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
