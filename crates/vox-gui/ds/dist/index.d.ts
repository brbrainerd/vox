import * as React from 'react';

export interface AxisMarkProps {
  /** Pixel size of the square mark. Default 24. */
  size?: number;
  className?: string;
}
/** The groma — a Roman surveyor's instrument — rendered as the Vox Axis app mark. Inherits color via currentColor; defaults to the imperial-gold accent. */
export function AxisMark(props: AxisMarkProps): React.JSX.Element;

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** `primary` uses the imperial-gold accent; `default` is the quiet surface button. */
  variant?: 'default' | 'primary';
}
/** Limes button. Cinzel caps with engraved letter-spacing; the primary variant carries the gold accent. */
export function Button(props: ButtonProps): React.JSX.Element;

export type StatusTone = 'neutral' | 'pass' | 'warn' | 'fail' | 'accent';
export interface StatusPillProps {
  /** Maps to the semantic status tokens (verdigris pass, terracotta warn, oxblood fail, verdigris accent). */
  tone?: StatusTone;
  label: string;
  /** When true, no leading dot is rendered. */
  hideDot?: boolean;
}
/** Compact status chip with a leading tone dot. */
export function StatusPill(props: StatusPillProps): React.JSX.Element;

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Render engraved corner ticks (top-left gold, top-right verdigris). */
  ticks?: boolean;
}
/** The Limes surface primitive: a bordered panel over a faint overlay with an inset highlight. */
export function Card(props: CardProps): React.JSX.Element;

export interface NavItemProps {
  label: string;
  active?: boolean;
  /** Optional leading glyph (icon element). */
  icon?: React.ReactNode;
  onClick?: () => void;
}
/** Sidebar navigation row; the active row gains a gold leading rail. */
export function NavItem(props: NavItemProps): React.JSX.Element;

export interface KpiTileProps {
  label: string;
  value: string | number;
  /** Signed delta; positive renders verdigris ▲, negative renders fail ▼. */
  delta?: number;
}
/** A single HUD metric tile: engraved label over a Cinzel tabular value. */
export function KpiTile(props: KpiTileProps): React.JSX.Element;
