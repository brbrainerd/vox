const { spawn } = require('child_process');
const serverPath = "c:\\Users\\Owner\\vox\\target\\debug\\vox.exe";

console.log("Spawning", serverPath, "mcp");
const child = spawn(serverPath, ['mcp']);

child.stdout.on('data', (d) => process.stdout.write("STDOUT: " + d.toString()));
child.stderr.on('data', (d) => process.stderr.write("STDERR: " + d.toString()));

child.on('error', (err) => console.log("SPAWN ERROR:", err));
child.on('exit', (code) => console.log("EXIT:", code));

setTimeout(() => {
    console.log("Sending initialize");
    child.stdin.write(JSON.stringify({
        jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "test", version: "1.0" } }
    }) + "\n");
}, 1000);

setTimeout(() => {
    console.log("Sending list tools");
    child.stdin.write(JSON.stringify({
        jsonrpc: "2.0", id: 2, method: "tools/list", params: {}
    }) + "\n");
}, 2000);

setTimeout(() => child.kill(), 4000);
