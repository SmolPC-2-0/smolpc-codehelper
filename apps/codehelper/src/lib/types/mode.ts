export type AppMode = 'code' | 'gimp' | 'blender' | 'writer' | 'calc' | 'impress';

export type ProviderKind = 'local' | 'mcp' | 'hybrid';

export interface ModeCapabilitiesDto {
	supportsTools: boolean;
	supportsUndo: boolean;
	showModelInfo: boolean;
	showHardwarePanel: boolean;
	showBenchmarkPanel: boolean;
	showExport: boolean;
	showContextControls: boolean;
}

export interface ModeConfigDto {
	id: AppMode;
	label: string;
	subtitle: string;
	icon: string;
	providerKind: ProviderKind;
	systemPromptKey: string;
	suggestions: string[];
	capabilities: ModeCapabilitiesDto;
}

