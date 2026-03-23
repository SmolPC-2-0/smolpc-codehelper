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
		subtitle: 'Ask for help with code, bugs, and new ideas',
		icon: 'code',
		providerKind: 'local',
		systemPromptKey: 'mode.code.default',
		suggestions: [
			'Explain what this loop does in simple words',
			'Help me find the bug in this function and explain the root cause.',
			'Write a function that checks whether a number is prime.'
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
		subtitle: 'Edit pictures in GIMP with plain-language requests',
		icon: 'image',
		providerKind: 'mcp',
		systemPromptKey: 'mode.gimp.default',
		suggestions: [
			'Make this image brighter without washing it out.',
			'Remove the background and keep the main subject.',
			'Crop this image to a square for a profile picture.'
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
		subtitle: 'Ask scene-aware Blender questions and get workflow help',
		icon: 'box',
		providerKind: 'hybrid',
		systemPromptKey: 'mode.blender.default',
		suggestions: [
			'Tell me what is in the current Blender scene.',
			'How do I smooth this model without losing its shape?',
			'Make the selected object look like red plastic.'
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
		subtitle: 'Create and edit Writer documents with guided help',
		icon: 'file-text',
		providerKind: 'mcp',
		systemPromptKey: 'mode.writer.default',
		suggestions: [
			'Create a document called lesson-plan.odt and add a title.',
			'Write a short introduction about renewable energy for school.',
			'Add a two-column table for topic and notes.'
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
		subtitle: 'Spreadsheet mode is visible, but not active yet',
		icon: 'table',
		providerKind: 'mcp',
		systemPromptKey: 'mode.calc.default',
		suggestions: [
			'Spreadsheet tools are planned, but this mode is not active yet.',
			'For documents, switch to Writer.',
			'For formulas or logic questions, Code mode may still help.'
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
		subtitle: 'Create and edit presentations slide by slide',
		icon: 'presentation',
		providerKind: 'mcp',
		systemPromptKey: 'mode.impress.default',
		suggestions: [
			'Create a 3-slide presentation about volcanoes.',
			'Add a title slide called Local AI in Schools.',
			'Rewrite slide 2 so it is shorter and clearer.'
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
