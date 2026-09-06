import React, { useState } from 'react';
import { Button } from '../../ui/Button';

interface TaskComposerProps {
  onSubmit: (text: string) => void;
  busy?: boolean;
}

export function TaskComposer({ onSubmit, busy = false }: TaskComposerProps) {
  const [text, setText] = useState('');

  const handleSubmit = (e?: React.FormEvent) => {
    e?.preventDefault();
    const trimmed = text.trim();
    if (!trimmed) return;
    onSubmit(trimmed);
    setText('');
  };

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-2 bg-white/2 border border-white/10 rounded-xl p-3">
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Add a task…"
        aria-label="Add a task"
        className="w-full min-h-[60px] bg-transparent text-[13px] text-zinc-100 placeholder:text-zinc-600 outline-hidden resize-none"
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSubmit();
          }
        }}
      />
      <div className="flex justify-end">
        <Button
          type="button"
          variant="primary"
          size="sm"
          onClick={() => handleSubmit()}
          disabled={busy || !text.trim()}
        >
          Add
        </Button>
      </div>
    </form>
  );
}
