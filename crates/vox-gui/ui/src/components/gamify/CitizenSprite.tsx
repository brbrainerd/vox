import React, { useEffect, useRef } from 'react';
import { useStore } from 'zustand';
import { projectIso, getZIndex } from '../../lib/projection';
import { useLudusStore, AgentState } from './store';

interface CitizenProps {
  id: string;
  name: string;
  tileWidth: number;
  tileHeight: number;
  offsetX: number;
  offsetY: number;
}

const MOOD_EMOJIS: Record<string, string> = {
  Happy: '😊',
  Tired: '🥱',
  Sad: '😢',
  Excited: '🤩',
  Exhausted: '😩',
};

export const CitizenSprite: React.FC<CitizenProps> = ({
  id,
  name,
  tileWidth,
  tileHeight,
  offsetX,
  offsetY,
}) => {
  const spriteRef = useRef<HTMLDivElement | null>(null);
  const agent = useStore(useLudusStore, (state) => state.agents[id]);
  const mood = agent?.mood || 'Happy';


  useEffect(() => {
    // 1. Lazy registration ONLY if the agent doesn't exist
    const store = useLudusStore.getState();
    if (!store.agents[id]) {
      store.updateAgent(id, { x: 2, y: 2, energy: 100, mood: 'Happy' });
    }

    const updateStyle = (agentState: AgentState) => {
      const el = spriteRef.current;
      if (!el) return;

      const { px, py } = projectIso(agentState.x, agentState.y, 0, tileWidth, tileHeight, offsetX, offsetY);
      const zIndex = getZIndex(agentState.x, agentState.y);

      el.style.transform = `translate3d(${px}px, ${py - 24}px, 0) translate(-50%, -50%)`;
      el.style.zIndex = zIndex.toString();
    };

    // 2. Immediate position update to align with mount and camera panning
    const initialAgent = useLudusStore.getState().agents[id] || { x: 2, y: 2, energy: 100, mood: 'Happy' as const };
    updateStyle(initialAgent);

    let prevAgentState = initialAgent;

    // 3. Subscription with change detection to avoid redundant style writes
    const unsubscribe = useLudusStore.subscribe((state) => {
      const agent = state.agents[id];
      if (!agent || agent === prevAgentState) return;
      prevAgentState = agent;
      updateStyle(agent);
    });

    return () => unsubscribe();
  }, [id, tileWidth, tileHeight, offsetX, offsetY]);

  return (
    <div
      ref={spriteRef}
      className="absolute flex flex-col items-center pointer-events-none"
      style={{ left: 0, top: 0 }}
    >
      <div className="text-[9px] bg-black/80 px-1 py-0.5 rounded-sm border border-blue-500/20 text-blue-400 font-mono scale-75 whitespace-nowrap mb-1">
        {name}
      </div>
      <div className="w-6 h-6 rounded-full bg-blue-500 flex items-center justify-center border border-white/20 shadow-lg">
        <span>{MOOD_EMOJIS[mood] || '👨‍💻'}</span>
      </div>
    </div>
  );
};
