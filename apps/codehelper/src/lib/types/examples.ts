export interface QuickExample {
	id: string;
	title: string;
	prompt: string;
	category: 'basics' | 'web' | 'algorithms' | 'debugging';
	icon?: string;
}

export const QUICK_EXAMPLES: QuickExample[] = [
	{
		id: 'calculator',
		title: 'Calculator Program',
		prompt: 'Create a simple calculator program that can add, subtract, multiply, and divide two numbers',
		category: 'basics'
	},
	{
		id: 'loops',
		title: 'Using Loops',
		prompt: 'Show me how to use for loops and while loops with examples',
		category: 'basics'
	},
	{
		id: 'website',
		title: 'Basic Website',
		prompt: 'Help me create a simple website with HTML, CSS, and JavaScript',
		category: 'web'
	},
	{
		id: 'fileio',
		title: 'File Operations',
		prompt: 'Show me how to read from and write to files',
		category: 'basics'
	},
	{
		id: 'sorting',
		title: 'Sorting Algorithms',
		prompt: 'Explain and show examples of different sorting algorithms',
		category: 'algorithms'
	},
	{
		id: 'debugging',
		title: 'Debug My Code',
		prompt: 'I have some code that is not working. Can you help me find and fix the bugs?',
		category: 'debugging'
	}
];
