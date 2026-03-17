#!/usr/bin/env node

import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

const source = process.env.SMOLPC_BUNDLED_MODEL_SOURCE;
if (!source) {
	console.error('SMOLPC_BUNDLED_MODEL_SOURCE is required');
	process.exit(1);
}

const repoRoot = path.resolve(import.meta.dirname, '..', '..', '..', '..');
const targetRoot = path.join(
	repoRoot,
	'apps',
	'codehelper',
	'src-tauri',
	'resources',
	'models',
	'qwen3-4b-instruct-2507'
);

await fs.rm(targetRoot, { recursive: true, force: true });
await fs.mkdir(path.dirname(targetRoot), { recursive: true });
await fs.cp(source, targetRoot, { recursive: true });
console.log(`Staged bundled model into ${targetRoot}`);
