import { Card } from '@vox-axis/limes';

const Stage = ({ children }: { children: any }) => (
  <div style={{ background: 'var(--color-bg-base)', padding: 24, display: 'flex', gap: 16 }}>
    {children}
  </div>
);

export const Plain = () => (
  <Stage>
    <Card style={{ width: 220 }}>
      <div className="ds-display" style={{ fontSize: 11, color: 'var(--color-text-primary)' }}>Surface</div>
      <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 6 }}>A bordered Limes panel.</div>
    </Card>
  </Stage>
);

export const WithTicks = () => (
  <Stage>
    <Card ticks style={{ width: 220 }}>
      <div className="ds-display" style={{ fontSize: 11, color: 'var(--color-text-primary)' }}>Operator</div>
      <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 6 }}>Engraved corner ticks.</div>
    </Card>
  </Stage>
);
