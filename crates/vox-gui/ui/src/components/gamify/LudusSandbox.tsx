import React, { useEffect, useRef, useState } from 'react';
import { projectIso } from '../../lib/projection';
import { HudPanels } from './HudPanels';
import { CitizenSprite } from './CitizenSprite';

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
  const plots = assignPlotCoordinates(files);
  const tileWidth = 64;
  const tileHeight = 32;
  const [camera, setCamera] = useState({ x: 400, y: 100, zoom: 1 });

  // Pre-render layout to offscreen canvas once
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

    // Draw buildings
    ctx.fillStyle = '#3b82f6';
    for (const [_, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
      ctx.beginPath();
      ctx.arc(px, py, 6, 0, 2 * Math.PI);
      ctx.fill();
    }
  }, [files]);

  // Main rendering loop blitting offscreen to onscreen with transforms
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let frameId: number;

    const render = () => {
      const offscreen = offscreenCanvasRef.current;
      if (!offscreen) {
        frameId = requestAnimationFrame(render);
        return;
      }
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.save();
      
      // Apply pan and zoom transforms
      ctx.translate(camera.x, camera.y);
      ctx.scale(camera.zoom, camera.zoom);

      // Copy buffer to screen (offset to align offscreen center with translation origin)
      ctx.drawImage(offscreen, -offscreen.width / 2, 0);
      
      ctx.restore();
      frameId = requestAnimationFrame(render);
    };

    render();
    return () => cancelAnimationFrame(frameId);
  }, [camera]);

  return (
    <div className="relative w-full h-[500px] bg-[#09090b] overflow-hidden border border-zinc-800 rounded-2xl">
      <canvas ref={canvasRef} width={800} height={500} className="absolute inset-0 w-full h-full" />
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
      <HudPanels
        treasuryValue={120}
        energy={90}
        speed={1}
        onSetSpeed={() => {}}
      />
    </div>
  );
};
