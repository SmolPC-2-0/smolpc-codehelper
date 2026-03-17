#!/usr/bin/env node

import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

const resourceRoots = ['python', 'gimp', 'blender', 'libreoffice', 'models'];

const repoRoot = path.resolve(import.meta.dirname, '..', '..', '..', '..');
const resourcesRoot = path.join(repoRoot, 'apps', 'codehelper', 'src-tauri', 'resources');

async function validateManifest(resourceName) {
	const resourceRoot = path.join(resourcesRoot, resourceName);
	const manifestPath = path.join(resourceRoot, 'manifest.json');
	const raw = await fs.readFile(manifestPath, 'utf8');
	const manifest = JSON.parse(raw);

	for (const field of ['version', 'source', 'expectedPaths', 'status']) {
		if (!(field in manifest)) {
			throw new Error(`${resourceName}: manifest missing required field '${field}'`);
		}
	}

	if (!Array.isArray(manifest.expectedPaths) || manifest.expectedPaths.length === 0) {
		throw new Error(`${resourceName}: expectedPaths must be a non-empty array`);
	}

	return {
		resourceName,
		manifest
	};
}

async function main() {
	const results = await Promise.all(resourceRoots.map(validateManifest));
	for (const { resourceName, manifest } of results) {
		console.log(
			`${resourceName}: version=${manifest.version} status=${manifest.status} expectedPaths=${manifest.expectedPaths.join(',')}`
		);
	}
}

main().catch((error) => {
	console.error(error.message);
	process.exitCode = 1;
});
