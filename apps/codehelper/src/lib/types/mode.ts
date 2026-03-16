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
		subtitle: '3D scene assistance for Blender workflows',
		icon: 'box',
		providerKind: 'hybrid',
		systemPromptKey: 'mode.blender.default',
		suggestions: ['Explain this scene', 'Create a simple material', 'Fix this modifier'],
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
		subtitle: 'Writing help for LibreOffice Writer',
		icon: 'file-text',
		providerKind: 'mcp',
		systemPromptKey: 'mode.writer.default',
		suggestions: ['Draft a paragraph', 'Rewrite this passage', 'Summarize this text'],
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
		subtitle: 'Spreadsheet help for LibreOffice Calc',
		icon: 'table',
		providerKind: 'mcp',
		systemPromptKey: 'mode.calc.default',
		suggestions: ['Explain this formula', 'Build a grade table', 'Clean this data'],
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
		subtitle: 'Presentation help for LibreOffice Slides',
		icon: 'presentation',
		providerKind: 'mcp',
		systemPromptKey: 'mode.impress.default',
		suggestions: ['Draft slide bullets', 'Turn notes into slides', 'Improve this outline'],
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
