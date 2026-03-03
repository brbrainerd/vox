/**
 * Check OpenCode Releases for breaking changes
 */
import { execSync } from 'child_process';
import fetch from 'node-fetch'; // if available

async function checkOpencodeVersion() {
    try {
        const localVersionStr = execSync('npm ls -g opencode-ai --json').toString();
        const json = JSON.parse(localVersionStr);
        const version = json.dependencies?.['opencode-ai']?.version;
        if (!version) return;

        console.log(`[Vox] Local OpenCode version: ${version}`);

        // Fetch latest version from NPM registry
        const response = await fetch('https://registry.npmjs.org/opencode-ai/latest');
        if (!response.ok) return;

        const latestData = await response.json();
        const latestVersion = latestData.version;

        if (latestVersion !== version) {
            console.log(`[Vox] ⚠️ A new version of OpenCode is available: ${latestVersion}`);
            console.log(`[Vox] Run: npm install -g opencode-ai@latest`);
        } else {
            console.log(`[Vox] OpenCode is up to date (${version})`);
        }

    } catch (e) {
        // silent
    }
}

checkOpencodeVersion();
