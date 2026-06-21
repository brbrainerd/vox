import { Button } from '@vox-axis/limes';

const Stage = ({ children }: { children: any }) => (
  <div style={{ background: 'var(--color-bg-base)', padding: 24, display: 'flex', gap: 12, alignItems: 'center' }}>
    {children}
  </div>
);

export const Primary = () => <Stage><Button variant="primary">Approve</Button></Stage>;
export const Default = () => <Stage><Button>Reject</Button></Stage>;
export const Pair = () => (
  <Stage>
    <Button variant="primary">Approve</Button>
    <Button>Reject</Button>
  </Stage>
);
