import React from 'react';

export interface NavItemProps {
  label: string;
  active?: boolean;
  /** Optional leading glyph (icon element). */
  icon?: React.ReactNode;
  onClick?: () => void;
}

/**
 * Sidebar navigation row. Cinzel caps, engraved letter-spacing. The active row
 * gains a gold rail on its leading edge — a deliberately quiet, glow-free
 * indicator (clarity over ornament).
 */
export function NavItem({ label, active = false, icon, onClick }: NavItemProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={['ds-nav', active ? 'ds-nav-active' : ''].filter(Boolean).join(' ')}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}
