export const APP_MODES = ['code', 'gimp', 'blender', 'writer', 'calc', 'impress'] as const;

export type AppMode = (typeof APP_MODES)[number];

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

export const FALLBACK_MODE_CONFIGS: ModeConfigDto[] = [
	{
		id: 'code',
		label: 'Code',
		subtitle: 'Codehelper workspace for fixes, explanations, and new code',
		icon: 'code',
		providerKind: 'local',
		systemPromptKey: 'mode.code.default',
		suggestions: [
			'Fix this bug and explain the root cause',
			'Write a function from this prompt',
			'Review this snippet for mistakes'
		],
		capabilities: {
			supportsTools: false,
			supportsUndo: false,
			showModelInfo: true,
			showHardwarePanel: true,
			showBenchmarkPanel: true,
			showExport: true,
			showContextControls: true
		}
	},
	{
		id: 'gimp',
		label: 'GIMP',
		subtitle: 'Live image editing help for GIMP through the unified assistant shell',
		icon: 'image',
		providerKind: 'mcp',
		systemPromptKey: 'mode.gimp.default',
		suggestions: [
			'Blur the top half of the image',
			'Crop this image to a square',
			'Rotate the image 90 degrees clockwise'
		],
		capabilities: {
			supportsTools: true,
			supportsUndo: true,
			showModelInfo: true,
			showHardwarePanel: true,
			showBenchmarkPanel: false,
			showExport: false,
			showContextControls: false
		}
	},
	{
		id: 'blender',
		label: 'Blender',
		subtitle: 'Live Blender tutoring with scene-aware guidance and Blender-doc grounding',
		icon: 'box',
		providerKind: 'hybrid',
		systemPromptKey: 'mode.blender.default',
		suggestions: [
			'What is in my scene right now?',
			'How do I add a bevel to the selected object?',
			'Explain what this modifier stack is doing'
		],
		capabilities: {
			supportsTools: true,
			supportsUndo: false,
			showModelInfo: true,
			showHardwarePanel: true,
			showBenchmarkPanel: false,
			showExport: false,
			showContextControls: false
		}
	},
	{
		id: 'writer',
		label: 'Writer',
		subtitle:
			'LibreOffice Writer scaffold in the unified shell; live document actions land in the activation phase',
		icon: 'file-text',
		providerKind: 'mcp',
		systemPromptKey: 'mode.writer.default',
		suggestions: [
			'Draft an introduction for this report',
			'Rewrite this paragraph for clarity',
			'Summarize these meeting notes'
		],
		capabilities: {
			supportsTools: true,
			supportsUndo: false,
			showModelInfo: true,
			showHardwarePanel: true,
			showBenchmarkPanel: false,
			showExport: false,
			showContextControls: false
		}
	},
	{
		id: 'calc',
		label: 'Calc',
		subtitle:
			'LibreOffice Calc scaffold in the unified shell; spreadsheet actions remain deferred for now',
		icon: 'table',
		providerKind: 'mcp',
		systemPromptKey: 'mode.calc.default',
		suggestions: [
			'Explain what this formula should do',
			'Outline a grade tracker sheet',
			'Suggest a clean table layout'
		],
		capabilities: {
			supportsTools: true,
			supportsUndo: false,
			showModelInfo: true,
			showHardwarePanel: true,
			showBenchmarkPanel: false,
			showExport: false,
			showContextControls: false
		}
	},
	{
		id: 'impress',
		label: 'Slides',
		subtitle:
			'LibreOffice Slides scaffold in the unified shell; live presentation actions land in the activation phase',
		icon: 'presentation',
		providerKind: 'mcp',
		systemPromptKey: 'mode.impress.default',
		suggestions: [
			'Turn these notes into slide bullets',
			'Suggest a three-slide deck outline',
			'Improve this presentation structure'
		],
		capabilities: {
			supportsTools: true,
			supportsUndo: false,
			showModelInfo: true,
			showHardwarePanel: true,
			showBenchmarkPanel: false,
			showExport: false,
			showContextControls: false
		}
	}
];
