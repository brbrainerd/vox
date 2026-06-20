import { StatusPill } from '@vox-axis/limes';

const Stage = ({ children }: { children: any }) => (
  <div style={{ background: 'var(--color-bg-base)', padding: 24, display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
    {children}
  </div>
);

export const Tones = () => (
  <Stage>
    <StatusPill tone="accent" label="Live" />
    <StatusPill tone="pass" label="Passed" />
    <StatusPill tone="warn" label="Doubted" />
    <StatusPill tone="fail" label="Failed" />
    <StatusPill label="Paused" />
  </Stage>
);
