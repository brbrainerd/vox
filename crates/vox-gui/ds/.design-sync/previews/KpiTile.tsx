import { KpiTile } from '@vox-axis/limes';

const Stage = ({ children }: { children: any }) => (
  <div style={{ background: 'var(--color-bg-base)', padding: 24, display: 'flex', gap: 16 }}>
    {children}
  </div>
);

export const Metrics = () => (
  <Stage>
    <KpiTile label="Active Agents" value={7} delta={2} />
    <KpiTile label="Queue Depth" value={12} delta={-3} />
    <KpiTile label="Mesh Peers" value={4} />
  </Stage>
);
