/**
 * Shared "jump to section" behavior: after switching to a target section and
 * letting layout settle (the caller schedules this inside a
 * `requestAnimationFrame`), scroll the destination element into view AND move
 * keyboard focus to it.
 *
 * Without the focus move, a keyboard/screen-reader user who activates a
 * jump-link (e.g. the Omnibar's "navigate + anchor" rows, or the LLM
 * settings banner's "add one under Keys & Secrets" link) gets no indication
 * anything happened: focus stays on the now-hidden trigger and nothing is
 * announced. The destination element must be focusable — give non-natively-
 * focusable containers (`<div>`, `<section>`, `<h2>`, ...) `tabIndex={-1}` in
 * their JSX so `.focus()` here actually works, without adding them to the
 * normal Tab order.
 */
export function scrollAndFocusAnchor(anchorId: string): void {
  const el = document.getElementById(anchorId);
  if (!el) return;
  el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  el.focus({ preventScroll: true });
}
