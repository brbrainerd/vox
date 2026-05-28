/**
 * Post-tsc smoke: host→webview zod schema accepts real message shapes.
 */
import { createRequire } from 'module';
import { fileURLToPath } from 'url';
import path from 'path';

const require = createRequire(import.meta.url);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const modPath = path.join(__dirname, '..', 'out', 'protocol', 'hostToWebviewMessages.js');

const { parseHostToWebviewMessage } = require(modPath);

const good = [
    { type: 'gamifyUpdate', value: { agent_count: 1 } },
    { type: 'capabilitiesUpdate', value: { toolCount: 5 } },
    { type: 'a2aTasks', value: [] },
    { type: 'planUpdate', value: '# Plan' },
];

for (const msg of good) {
    if (!parseHostToWebviewMessage(msg)) {
        console.error('Expected parse OK:', msg);
        process.exit(1);
    }
}

if (parseHostToWebviewMessage({ type: 'not-a-real-host-message' })) {
    console.error('Expected parse fail for unknown type');
    process.exit(1);
}

console.log('smoke-host-messages: OK');
