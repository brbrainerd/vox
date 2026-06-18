import React, { useEffect, useRef } from 'react';
import { projectIso, getZIndex } from '../../lib/projection';
import { useLudusStore } from './store';

interface CitizenProps {
  id: string;
  name: string;
  tileWidth: number;
  tileHeight: number;
  offsetX: number;
  offsetY: number;
}

export const CitizenSprite: React.FC<CitizenProps> = ({
  id,
  name,
  tileWidth,
  tileHeight,
  offsetX,
  offsetY,
}) => {
  const spriteRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    // Register agent in the store
    useLudusStore.getState().updateAgent(id, { x: 2, y: 2, energy: 100, mood: 'Happy' });

    // Subscribe directly to store updates for this specific agent id
    const unsubscribe = useLudusStore.subscribe((state) => {
      const agent = state.agents[id];
      const el = spriteRef.current;
      if (!agent || !el) return;

      // Projection translation
      const { px, py } = projectIso(agent.x, agent.y, 0, tileWidth, tileHeight, offsetX, offsetY);
      const zIndex = getZIndex(agent.x, agent.y);

      // Direct styling updates bypassing React render cycle
      el.style.transform = `translate3d(${px}px, ${py - 24}px, 0) translate(-50%, -50%)`;
      el.style.zIndex = zIndex.toString();
    });

    return () => unsubscribe();
  }, [id, tileWidth, tileHeight, offsetX, offsetY]);

  return (
    <div
      ref={spriteRef}
      className="absolute flex flex-col items-center pointer-events-none transition-transform duration-75"
      style={{ left: 0, top: 0, zIndex: 0 }}
    >
      <div className="text-[9px] bg-black/80 px-1 py-0.5 rounded border border-blue-500/20 text-blue-400 font-mono scale-75 whitespace-nowrap mb-1">
        {name}
      </div>
      <div className="w-6 h-6 rounded-full bg-blue-500 flex items-center justify-center border border-white/20 shadow-lg">
        <span>👨‍💻</span>
      </div>
    </div>
  );
};
