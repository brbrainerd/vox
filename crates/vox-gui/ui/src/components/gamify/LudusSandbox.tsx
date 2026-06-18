import React, { useEffect, useRef, useState } from 'react';
import { useStore } from 'zustand';
import { projectIso } from '../../lib/projection';
import { HudPanels } from './HudPanels';
import { CitizenSprite } from './CitizenSprite';
import { useLudusStore } from './store';
import { listenAgentEvents, AgentEventFrame } from '../../transport';

export interface GridPlot {
  x: number;
  y: number;
  z: number;
}

export function assignPlotCoordinates(files: string[]): Record<string, GridPlot> {
  const plots: Record<string, GridPlot> = {};
  let index = 0;
  for (const file of files) {
    const r = Math.floor(Math.sqrt(index));
    const angle = index * 2.4;
    const x = Math.round(4 + r * Math.cos(angle));
    const y = Math.round(4 + r * Math.sin(angle));
    plots[file] = { x, y, z: 0 };
    index++;
  }
  return plots;
}

interface SandboxProps {
  files: string[];
}

export const LudusSandbox: React.FC<SandboxProps> = ({ files }) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const offscreenCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const plots = React.useMemo(() => assignPlotCoordinates(files), [files]);
  const tileWidth = 64;
  const tileHeight = 32;
  const [camera, setCamera] = useState({ x: 400, y: 100, zoom: 1 });
  const [bubble, setBubble] = useState<string | null>(null);

  // Select building status to trigger re-renders on quality changes
  const buildings = useStore(useLudusStore, (state) => state.buildings);
  const focusedFile = useStore(useLudusStore, (state) => state.focusedFile);
  const agentTasks = useStore(useLudusStore, (state) => state.agentTasks);

  // Auto-center camera target when focused file changes
  useEffect(() => {
    if (!focusedFile) return;
    const plot = plots[focusedFile];
    if (!plot) return;
    const centerOffsetX = 1000; // Center offset of offscreen canvas
    const centerOffsetY = 100;
    const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
    
    const targetZoom = 1.2;
    // Pan camera to center coordinates: cameraX = viewportWidth/2 - (px - centerOffsetX) * zoom, cameraY = viewportHeight/2 - py * zoom
    setCamera({
      x: 400 - (px - centerOffsetX) * targetZoom,
      y: 250 - py * targetZoom,
      zoom: targetZoom
    });
  }, [focusedFile, plots]);



  // Pre-render layout to offscreen canvas
  useEffect(() => {
    const canvas = document.createElement('canvas');
    canvas.width = 2000;
    canvas.height = 2000;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    offscreenCanvasRef.current = canvas;

    // Draw isometric grid
    ctx.strokeStyle = '#27272a';
    ctx.lineWidth = 1;
    const centerOffsetX = canvas.width / 2;
    const centerOffsetY = 100;

    for (let x = 0; x < 24; x++) {
      for (let y = 0; y < 24; y++) {
        const { px, py } = projectIso(x, y, 0, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
        ctx.beginPath();
        ctx.moveTo(px, py - tileHeight / 2);
        ctx.lineTo(px + tileWidth / 2, py);
        ctx.lineTo(px, py + tileHeight / 2);
        ctx.lineTo(px - tileWidth / 2, py);
        ctx.closePath();
        ctx.stroke();
      }
    }

    // Draw buildings with weeds/cracks overlays
    const activeScaffolds = new Set(Object.values(agentTasks).map(t => t.filePath));
    for (const [filePath, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
      const bState = buildings[filePath] || { warnings: 0, errors: 0 };

      // Base building: Red for errors, Blue for normal
      ctx.fillStyle = bState.errors > 0 ? '#ef4444' : '#3b82f6';
      ctx.beginPath();
      ctx.arc(px, py, 6, 0, 2 * Math.PI);
      ctx.fill();

      // Render Weeds (Warnings)
      if (bState.warnings > 0) {
        ctx.fillStyle = '#10b981';
        ctx.fillRect(px - 10, py + 2, 4, 4);
        ctx.fillRect(px + 6, py + 2, 4, 4);
      }

      if (activeScaffolds.has(filePath)) {
        // Draw wooden construction scaffolding
        ctx.strokeStyle = '#b45309'; // Brown/wood color
        ctx.lineWidth = 1.5;
        // Draw crossed pillars
        ctx.beginPath();
        ctx.moveTo(px - 14, py - 6);
        ctx.lineTo(px + 14, py + 6);
        ctx.moveTo(px - 14, py + 6);
        ctx.lineTo(px + 14, py - 6);
        ctx.stroke();
      }
    }

    // Render active agent tasks glows
    for (const task of Object.values(agentTasks)) {
      const plot = plots[task.filePath];
      if (!plot) continue;
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
      ctx.strokeStyle = '#ef4444';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(px, py, 14, 0, 2 * Math.PI);
      ctx.stroke();
    }
  }, [files, plots, buildings, agentTasks]);

  // Render offscreen canvas to onscreen viewport on camera or layout updates
  useEffect(() => {
    const canvas = canvasRef.current;
    const offscreen = offscreenCanvasRef.current;
    if (!canvas || !offscreen) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.save();
    
    // Apply pan and zoom transforms
    ctx.translate(camera.x, camera.y);
    ctx.scale(camera.zoom, camera.zoom);

    // Copy buffer to screen
    ctx.drawImage(offscreen, -offscreen.width / 2, 0);
    
    ctx.restore();
  }, [camera, files, buildings, agentTasks]);

  // Listen to live agent execution events from Tauri
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    listenAgentEvents((event: AgentEventFrame) => {
      const store = useLudusStore.getState();
      if (event.kind.type === 'file_edited') {
        const filePath = event.kind.path || 'crates/vox-db/src/lib.rs';
        // Mock compile warning changes
        store.updateBuilding(filePath, { warnings: 1 });

        // Trigger speech bubble
        setBubble('Hammering out features! 🛠️');
        setTimeout(() => {
          if (active) setBubble(null);
        }, 3000);
      }
    }).then((unlistenFn) => {
      if (!active) {
        unlistenFn();
      } else {
        unlisten = unlistenFn;
      }
    }).catch(() => {});

    return () => {
      active = false;
      if (unlisten) unlisten();
    };
  }, []);

  const handleCanvasClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const clickX = e.clientX - rect.left;
    const clickY = e.clientY - rect.top;

    // Convert screen coordinates to world coordinates based on camera transform
    const worldX = (clickX - camera.x) / camera.zoom;
    const worldY = (clickY - camera.y) / camera.zoom;

    const centerOffsetX = 1000;
    const centerOffsetY = 100;

    // Find clicked building within click radius threshold
    for (const [filePath, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
      const dx = worldX - (px - 1000);
      const dy = worldY - py;
      const distance = Math.sqrt(dx * dx + dy * dy);
      if (distance < 20) {
        useLudusStore.getState().setFocusedFile(filePath);
        break;
      }
    }
  };

  return (
    <div className="relative w-full h-[500px] bg-[#09090b] overflow-hidden border border-zinc-800 rounded-2xl">
      <canvas
        ref={canvasRef}
        width={800}
        height={500}
        className="absolute inset-0 w-full h-full cursor-pointer"
        onClick={handleCanvasClick}
      />
      <div className="absolute inset-0 pointer-events-none">
        <CitizenSprite
          id="dev"
          name="Developer"
          tileWidth={tileWidth}
          tileHeight={tileHeight}
          offsetX={camera.x}
          offsetY={camera.y}
        />
      </div>
      {bubble && (
        <div className="absolute top-[180px] left-[380px] bg-zinc-900 border border-zinc-700 text-zinc-100 text-[10px] px-2 py-1 rounded shadow-lg animate-bounce whitespace-nowrap z-20">
          💬 {bubble}
        </div>
      )}
      {Object.values(agentTasks).map((task) => {
        const plot = plots[task.filePath];
        if (!plot) return null;
        const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, 1000, 100);
        const left = camera.x + (px - 1000) * camera.zoom;
        const top = camera.y + py * camera.zoom;
        return (
          <div
            key={task.taskId}
            data-testid="task-clipboard"
            className="absolute pointer-events-auto cursor-pointer bg-zinc-900 border border-zinc-700 p-1 rounded text-xs select-none z-10"
            style={{ left: left - 10, top: top - 20 }}
          >
            📋
          </div>
        );
      })}
      <HudPanels
        treasuryValue={120}
        energy={90}
        speed={1}
        onSetSpeed={() => {}}
      />
    </div>
  );
};
