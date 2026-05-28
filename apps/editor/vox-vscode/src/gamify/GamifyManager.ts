import * as vscode from 'vscode';
import { VoxMcpClient } from '../core/VoxMcpClient';
import type { GamifyState } from '../types';
import { ConfigManager } from '../core/ConfigManager';

export class GamifyManager {
    private _state: GamifyState = {
        level: 1, xp: 0, crystals: 0, streak: 0, streak_frozen: false,
    };
    private _pollTimer?: NodeJS.Timeout;
    private _onUpdate: (state: GamifyState) => void;
    private _seenAchievements = new Set<string>();
    private readonly _statusBar?: vscode.StatusBarItem;

    constructor(
        private readonly _mcp: VoxMcpClient,
        onUpdate: (state: GamifyState) => void,
        ludusStatusBar?: vscode.StatusBarItem,
    ) {
        this._onUpdate = onUpdate;
        this._statusBar = ludusStatusBar;
        if (this._statusBar) {
            this._statusBar.command = 'vox.focusSidebar';
            this._statusBar.hide();
        }
    }

    start(): void {
        if (!ConfigManager.gamifyShowHud) return;
        this._pollTimer = setInterval(() => this._poll(), 30_000);
        this._poll();
    }

    stop(): void {
        clearInterval(this._pollTimer);
    }

    private async _poll(): Promise<void> {
        if (!this._mcp.connected) return;
        const status = await this._mcp.orchestratorStatus();
        if (!status) return;

        const prev = this._state;
        this._state = {
            level: (status as GamifyState).level ?? prev.level,
            xp: (status as GamifyState).xp ?? prev.xp,
            crystals: (status as GamifyState).crystals ?? prev.crystals,
            streak: (status as GamifyState).streak ?? prev.streak,
            streak_frozen: (status as GamifyState).streak_frozen ?? prev.streak_frozen,
            companion_name: (status as GamifyState).companion_name,
            companion_mood: (status as GamifyState).companion_mood,
            achievements: (status as GamifyState).achievements,
        };

        // Level up notification
        if (this._state.level > prev.level) {
            vscode.window.showInformationMessage(
                `🎉 Level Up! You are now Level ${this._state.level}`,
                'View HUD'
            ).then(sel => {
                if (sel === 'View HUD') vscode.commands.executeCommand('vox.focusSidebar');
            });
        }

        // New achievement notification
        for (const ach of (this._state.achievements ?? [])) {
            if (ach.unlocked_at && !this._seenAchievements.has(ach.id)) {
                this._seenAchievements.add(ach.id);
                vscode.window.showInformationMessage(
                    `🏆 Achievement Unlocked: ${ach.icon} ${ach.name}`,
                    'View Achievements'
                ).then(sel => {
                    if (sel === 'View Achievements') vscode.commands.executeCommand('vox.focusSidebar');
                });
            }
        }

        this._onUpdate(this._state);

        if (this._statusBar) {
            if (!ConfigManager.gamifyShowHud) {
                this._statusBar.hide();
            } else {
                const st = this._streakLabel();
                this._statusBar.text = `$(flame) Lv ${this._state.level} · ${st}`;
                this._statusBar.tooltip = new vscode.MarkdownString(
                    `**Ludus** — ${this._state.xp} XP, ${this._state.crystals ?? 0} crystals\n\n[Open sidebar](command:vox.focusSidebar)`,
                );
                this._statusBar.tooltip.isTrusted = true;
                this._statusBar.show();
            }
        }
    }

    private _streakLabel(): string {
        if (this._state.streak_frozen) return `${this._state.streak}d (frozen)`;
        return `${this._state.streak}d`;
    }

    get state(): GamifyState { return this._state; }
}
