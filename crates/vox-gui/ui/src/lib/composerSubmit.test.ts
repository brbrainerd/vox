import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { userAppendInput } from './composerSubmit';

describe('userAppendInput', () => {
  it('marks composer messages as already submitted so the backend secretary skips them', () => {
    const input = userAppendInput(
      'sess-1',
      'refactor the login flow so it stops redirecting users after auth',
    );
    expect(input.already_submitted).toBe(true);
    expect(input.session_id).toBe('sess-1');
    expect(input.role).toBe('user');
    expect(input.task_id).toBeNull();
    expect(input.content).toBe('refactor the login flow so it stops redirecting users after auth');
  });

  it('stringifies missing descriptions to the empty string', () => {
    expect(userAppendInput('sess-1', undefined).content).toBe('');
    expect(userAppendInput('sess-1', null).content).toBe('');
  });
});

// F28 wiring guard: the payload-builder tests above cannot catch a reverted
// or skipped call-site edit — revert the App.tsx change and they stay green
// while the double-submit returns. Mirror the Task 2 idiom (ErrorBoundary
// .test.tsx reads main.tsx via readFileSync) and pin the composer persist
// call to the new payload builder.
describe('App.tsx composer persist wiring (C2)', () => {
  it('routes the chat_append_message payload through userAppendInput', () => {
    const app = readFileSync(resolve(__dirname, '../App.tsx'), 'utf8');
    const call = app.indexOf("invoke('chat_append_message'");
    expect(call).toBeGreaterThan(-1);
    // The FIRST chat_append_message invoke in App.tsx is the composer
    // user-persist path (the later one at ~849 persists assistant replies);
    // its input must come from userAppendInput(...).
    expect(app.slice(call, call + 220)).toContain('userAppendInput(sessionId');
    // The old inline payload (which never carried already_submitted) is gone.
    expect(app).not.toContain("{ session_id: sessionId, role: 'user'");
  });
});
