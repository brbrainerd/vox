import { AxisMark } from '@vox-axis/limes';

const Stage = ({ children }: { children: any }) => (
  <div style={{ background: 'var(--color-bg-base)', padding: 24, display: 'flex', gap: 20, alignItems: 'center' }}>
    {children}
  </div>
);

export const Default = () => <Stage><AxisMark /></Stage>;
export const Sizes = () => (
  <Stage>
    <AxisMark size={20} />
    <AxisMark size={32} />
    <AxisMark size={48} />
  </Stage>
);
