import { NavItem } from '@vox-axis/limes';

const Stage = ({ children }: { children: any }) => (
  <div style={{ background: 'var(--color-bg-base)', padding: 24, width: 220, display: 'flex', flexDirection: 'column', gap: 2 }}>
    {children}
  </div>
);

export const Rail = () => (
  <Stage>
    <NavItem label="Chat" active />
    <NavItem label="Agents" />
    <NavItem label="Runs & Approvals" />
    <NavItem label="Settings" />
  </Stage>
);
