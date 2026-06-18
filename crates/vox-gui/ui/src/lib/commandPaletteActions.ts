export type NavigateToFn = (viewKey: string) => void;
export type FocusComposerFn = () => void;
export type ScheduleFocusFn = (fn: () => void) => void;

const defaultScheduleFocus: ScheduleFocusFn = (fn) => {
  requestAnimationFrame(() => {
    requestAnimationFrame(fn);
  });
};

/** ⌘K "Submit new task…" — open Chat and focus the Loquela composer. */
export function handleSubmitTaskAction(
  navigateTo: NavigateToFn,
  focusComposer: FocusComposerFn,
  scheduleFocus: ScheduleFocusFn = defaultScheduleFocus,
): void {
  navigateTo('chat');
  scheduleFocus(focusComposer);
}
