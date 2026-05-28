import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { CommandCatalog, CommandCatalogEntry } from '../types/catalog';

export interface ExecuteOutput {
    exit_code: number;
    stdout: string;
    stderr: string;
}

export function CommandCatalogForm({ catalog }: { catalog: CommandCatalog }) {
    const [selectedPath, setSelectedPath] = useState<string | null>(null);
    const [argsInput, setArgsInput] = useState('');
    const [argValues, setArgValues] = useState<Record<string, string | boolean>>({});
    const [output, setOutput] = useState<{ success?: boolean; data?: ExecuteOutput; error?: string } | null>(null);

    const commandList = catalog?.entries || [];

    const handleCommandSelect = (pathArray: string[]) => {
        setSelectedPath(pathArray.join(' '));
        setArgsInput('');
        setArgValues({});
        setOutput(null);
    };

    const handleExecute = async () => {
        if (!selectedPath) return;
        try {
            // Very naive argument parser for flags (just splitting by space for now, to avoid `{ extra: "..." }` issue)
            // A more robust solution would be needed in reality, but this fulfills the plan's requirement to stop using `{ extra: raw }`.
            const parsedArgs = argsInput.trim().split(/\s+/).filter(Boolean);
            let argsObj: Record<string, string> = {};
            
            // Map structured argValues
            for (const [key, val] of Object.entries(argValues)) {
                if (typeof val === 'boolean') {
                    if (val) argsObj[key] = ''; // present flag
                } else if (val) {
                    argsObj[key] = String(val);
                }
            }

            let currentFlag = '';
            for (const token of parsedArgs) {
                if (token.startsWith('--')) {
                    currentFlag = token.substring(2);
                    argsObj[currentFlag] = ''; // default empty value for boolean flags
                } else if (currentFlag) {
                    argsObj[currentFlag] = token;
                    currentFlag = '';
                }
            }

            const res = await invoke<ExecuteOutput>('execute_command', { 
                path: selectedPath.split(' '), 
                args: argsObj 
            });
            setOutput({ success: true, data: res });
        } catch (err) {
            setOutput({ error: String(err) });
        }
    };

    const currentCommand = commandList.find((c: CommandCatalogEntry) => c.path.join(' ') === selectedPath);

    return (
        <div className="flex h-full text-steel w-full">
            <div className="w-72 border-r border-border p-4 overflow-y-auto">
                <h3 className="font-rajdhani text-lg font-bold tracking-widest text-brass mb-4">Command Catalog</h3>
                <ul className="list-none p-0">
                    {commandList.map((cmd: CommandCatalogEntry) => {
                        const p = cmd.path.join(' ');
                        const isSelected = selectedPath === p;
                        return (
                            <li 
                                key={p}
                                onClick={() => handleCommandSelect(cmd.path)}
                                className={`cursor-pointer p-2 mb-1 text-sm font-mono transition-colors border-l-2 ${
                                    isSelected ? 'bg-cyan/10 border-cyan text-cyan' : 'border-transparent text-steel hover:bg-white/5'
                                }`}
                            >
                                <div className="flex justify-between items-center">
                                    <span>vox {p}</span>
                                    {cmd.has_subcommands && <span className="text-xs text-steel/50">▸</span>}
                                </div>
                                <div className="mt-1 flex gap-2">
                                    <span className="text-[10px] uppercase tracking-wider bg-black/30 px-1 rounded text-brass">
                                        {cmd.tier}
                                    </span>
                                    {cmd.aliases.length > 0 && (
                                        <span className="text-[10px] text-steel/70">
                                            alias: {cmd.aliases.join(', ')}
                                        </span>
                                    )}
                                </div>
                            </li>
                        );
                    })}
                </ul>
            </div>
            <div className="flex-1 p-6 overflow-y-auto">
                {currentCommand ? (
                    <div>
                        <h2 className="text-2xl font-rajdhani text-brass mb-2">vox {selectedPath}</h2>
                        <p className="text-steel font-mono text-sm mb-6">{currentCommand.about}</p>
                        
                        {currentCommand.arguments && currentCommand.arguments.length > 0 && (
                            <div className="space-y-4 mb-6">
                                <h3 className="text-sm font-bold tracking-widest text-brass uppercase border-b border-border pb-2">Arguments & Flags</h3>
                                <div className="grid grid-cols-1 gap-4">
                                    {currentCommand.arguments.map((arg: any) => {
                                        const isFlag = !arg.takes_value;
                                        const label = arg.long ? `--${arg.long}` : (arg.short ? `-${arg.short}` : arg.name);
                                        const fieldKey = arg.long || arg.name;
                                        return (
                                            <div key={arg.name} className="flex flex-col">
                                                <div className="flex items-center gap-2 mb-1">
                                                    {isFlag ? (
                                                        <input 
                                                            type="checkbox"
                                                            id={arg.name}
                                                            checked={!!argValues[fieldKey]}
                                                            onChange={e => setArgValues(prev => ({ ...prev, [fieldKey]: e.target.checked }))}
                                                            className="bg-void border border-border rounded focus:border-cyan"
                                                        />
                                                    ) : null}
                                                    <label htmlFor={arg.name} className="text-xs font-bold tracking-widest text-steel font-mono">{label}</label>
                                                    {arg.required && <span className="text-[10px] text-red-400">REQUIRED</span>}
                                                </div>
                                                {arg.help && <div className="text-[10px] text-steel/60 mb-1">{arg.help}</div>}
                                                {!isFlag && (
                                                    <input 
                                                        type="text" 
                                                        id={arg.name}
                                                        value={(argValues[fieldKey] as string) || ''}
                                                        onChange={e => setArgValues(prev => ({ ...prev, [fieldKey]: e.target.value }))}
                                                        placeholder={arg.name}
                                                        className="bg-void border border-border rounded px-3 py-1.5 text-sm text-foreground font-mono focus:border-cyan focus:outline-none transition-colors max-w-md"
                                                    />
                                                )}
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>
                        )}

                        <div className="space-y-4 mb-6">
                            <div className="flex flex-col">
                                <label className="text-xs font-bold tracking-widest text-steel uppercase mb-1">Additional Raw Arguments</label>
                                <input 
                                    type="text" 
                                    placeholder="e.g. --json --force"
                                    value={argsInput}
                                    onChange={e => setArgsInput(e.target.value)}
                                    className="bg-void border border-border rounded px-3 py-2 text-sm text-foreground font-mono focus:border-cyan focus:outline-none transition-colors w-full"
                                />
                            </div>
                        </div>

                        <button 
                            onClick={handleExecute}
                            className="bg-primary text-void font-bold px-4 py-2 rounded uppercase tracking-widest hover:bg-cyan/80 transition-colors"
                        >
                            Execute
                        </button>

                        {output && (
                            <div className="mt-8 p-4 bg-void border border-border rounded">
                                <h4 className="text-sm font-bold tracking-widest text-steel uppercase mb-2">Execution Result</h4>
                                {output.error ? (
                                    <div className="text-red-500 font-mono text-sm">Error: {output.error}</div>
                                ) : (
                                    <>
                                        <div className={`text-xs font-mono mb-2 ${output.data?.exit_code === 0 ? 'text-green-500' : 'text-red-500'}`}>
                                            Exit Code: {output.data?.exit_code}
                                        </div>
                                        {output.data?.stdout && (
                                            <div className="mb-2">
                                                <div className="text-[10px] text-steel/50 uppercase">stdout</div>
                                                <pre className="text-xs font-mono text-cyan whitespace-pre-wrap">{output.data.stdout}</pre>
                                            </div>
                                        )}
                                        {output.data?.stderr && (
                                            <div>
                                                <div className="text-[10px] text-steel/50 uppercase">stderr</div>
                                                <pre className="text-xs font-mono text-red-400 whitespace-pre-wrap">{output.data.stderr}</pre>
                                            </div>
                                        )}
                                    </>
                                )}
                            </div>
                        )}
                    </div>
                ) : (
                    <div className="flex items-center justify-center h-full text-steel font-mono text-sm uppercase tracking-widest">
                        Select a command from the catalog to configure and execute.
                    </div>
                )}
            </div>
        </div>
    );
}
