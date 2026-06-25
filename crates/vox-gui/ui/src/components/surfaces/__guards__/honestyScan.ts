export type Violation = {
  file: string;
  line: number;
  kind: 'placeholder' | 'dead-handler';
  snippet: string;
};

const PLACEHOLDER =
  /\b(not\s+(yet\s+)?(implemented|wired|available|working|hooked|connected)|coming\s+soon|placeholder)\b/i;
// empty or brace-only arrow body: onClick={() => {}} / onClick={()=>{ }}
const DEAD_HANDLER =
  /on(Click|Submit|Change|Press)=\{\s*\(\s*[^)]*\)\s*=>\s*\{\s*\}\s*\}/;

export function scanSource(file: string, text: string): Violation[] {
  const out: Violation[] = [];
  text.split('\n').forEach((raw, i) => {
    const line = i + 1;
    if (PLACEHOLDER.test(raw)) out.push({ file, line, kind: 'placeholder', snippet: raw.trim() });
    if (DEAD_HANDLER.test(raw)) out.push({ file, line, kind: 'dead-handler', snippet: raw.trim() });
  });
  return out;
}
