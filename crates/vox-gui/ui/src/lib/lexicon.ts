// Single source of truth for the en/la dimension of user-facing labels.
// `la` omitted = proper noun: keeps `en` in both modes.
export type Lang = 'en' | 'la';
export interface LexEntry { en: string; la?: string }

export const LEXICON: Record<string, LexEntry> = {
  chat: { en: 'Chat', la: 'Loquela' },
  agents: { en: 'Agents', la: 'Agentes' },
  runs: { en: 'Runs', la: 'Cursus' },
  'nav:runs': { en: 'Runs & Approvals', la: 'Cursus & Probationes' },
  workspace: { en: 'Workspace', la: 'Officina' },
  commands: { en: 'Commands', la: 'Mandata' },
  search: { en: 'Search', la: 'Quaestio' },
  knowledge: { en: 'Knowledge', la: 'Scientia' },
  compute: { en: 'Compute', la: 'Computatio' },
  mercatus: { en: 'Market', la: 'Mercatus' },
  settings: { en: 'Settings', la: 'Configuratio' },
  activity: { en: 'Activity', la: 'Acta' },
  approvals: { en: 'Approvals', la: 'Probationes' },
  'archive-panel': { en: 'Archive Panel', la: 'Tabularium' },
  browser: { en: 'Browser', la: 'Explorator' },
  catalog: { en: 'Catalog', la: 'Catalogus' },
  claims: { en: 'Claims', la: 'Assertiones' },
  console: { en: 'Console', la: 'Terminus' },
  coverage: { en: 'Coverage', la: 'Tegmen' },
  dashboard: { en: 'Dashboard', la: 'Specula' },
  'discovery-inbox': { en: 'Discovery Inbox', la: 'Inventa Nova' },
  'discovery-review': { en: 'Discovery Review', la: 'Recensio Inventorum' },
  flow: { en: 'Flow', la: 'Fluxus' },
  gamify: { en: 'Gamify', la: 'Ludus' },
  harness: { en: 'Harness', la: 'Apparatus' },
  matrix: { en: 'Routing', la: 'Itinera' },
  memory: { en: 'Memory', la: 'Memoria' },
  mens: { en: 'Mens' },
  mesh: { en: 'Mesh', la: 'Rete' },
  models: { en: 'Models', la: 'Exemplaria' },
  'needs-you': { en: 'Needs You', la: 'Postulata' },
  oratio: { en: 'Oratio' },
  policies: { en: 'Policies', la: 'Regulae' },
  populi: { en: 'Populi' },
  publications: { en: 'Publications', la: 'Edita' },
  repository: { en: 'Repository', la: 'Repositorium' },
  research: { en: 'Research', la: 'Investigatio' },
  review: { en: 'Review', la: 'Recensio' },
  scientia: { en: 'Findings', la: 'Inventa' },
  skills: { en: 'Skills', la: 'Artes' },
  'sub-agents': { en: 'Sub-Agents', la: 'Subagentes' },
  'vox-search': { en: 'Search Index', la: 'Index' },
  tasks: { en: 'Tasks', la: 'Munera' },
  // Phase 2 heading slugs (agents/needs-you/mesh reuse the nav keys above)
  'mc-mission': { en: 'Mission Control', la: 'Praefectura' },
  'vg-corpus-health': { en: 'Graphify Corpus Health', la: 'Sanitas Corporis' },
  'sci-claims': { en: 'Findings Claims', la: 'Assertiones Inventorum' },
  'sci-home': { en: 'Vox Findings', la: 'Inventa Vox' },
  'dash-stream': { en: 'The Stream', la: 'Flumen' },
  'dash-telemetry': { en: 'System · Telemetry & Alerts', la: 'Systema · Telemetria' },
  'dash-active-agents': { en: 'Active Agents', la: 'Agentes Activi' },
  'appr-pending': { en: 'Pending Approvals', la: 'Probationes Pendentes' },
  'cat-center': { en: 'Command Center', la: 'Praefectura Mandatorum' },
  'chat-sessions': { en: 'Sessions', la: 'Sessiones' },
  'chat-execution': { en: 'Execution', la: 'Executio' },
  'con-discovery': { en: 'Discovery', la: 'Investigatio' },
  'cov-surface': { en: 'Surface Coverage', la: 'Tegmen Superficiei' },
  'set-display': { en: 'Display', la: 'Aspectus' },
  'mat-routing': { en: 'Routing Policies', la: 'Regulae Itinerum' },
  'mat-axis': { en: 'Axis Inspector', la: 'Inspector Axis' },
  'mem-shards': { en: 'Memory Shards', la: 'Fragmenta Memoriae' },
  'pub-pipeline': { en: 'Publication Pipeline', la: 'Processus Editionis' },
  'repo-harness': { en: 'Repository Harness', la: 'Apparatus Repositorii' },
  'gamification': { en: 'Gamification', la: 'Ludificatio' },
  // SettingsView section headings
  'set-mesh-peers': { en: 'Mesh & peers', la: 'Rete & Socii' },
  'set-signing': { en: 'Signing keys', la: 'Claves Signatae' },
  'set-secrets': { en: 'Keys & Secrets', la: 'Claves & Arcana' },
  'set-runtime': { en: 'Runtime', la: 'Tempus Executionis' },
  'set-llm': { en: 'LLM & providers', la: 'LLM & Provisores' },
  'set-orchestrator': { en: 'Orchestrator' }, // proper noun
  'set-scaling': { en: 'Scaling', la: 'Scalatio' },
  'set-routing': { en: 'Model routing', la: 'Directio Exemplarium' },
  'set-telemetry': { en: 'Telemetry', la: 'Telemetria' },
  'set-keybinds': { en: 'Keybinds', la: 'Vincula Clavium' },
  'set-theme': { en: 'Theme', la: 'Thema' },
  'group:operate': { en: 'Operate', la: 'Operatio' },
  'group:develop': { en: 'Develop', la: 'Fabrica' },
  'group:knowledge': { en: 'Knowledge', la: 'Scientia' },
  'group:compute': { en: 'Compute', la: 'Computatio' },
  'group:system': { en: 'System', la: 'Systema' },
};

export function pick(entry: LexEntry, lang: Lang): string {
  return lang === 'la' ? (entry.la ?? entry.en) : entry.en;
}

// Resolve by id; unknown ids return the id itself (safe for not-yet-seeded surfaces).
export function labelFor(id: string, lang: Lang): string {
  const e = LEXICON[id];
  return e ? pick(e, lang) : id;
}

// Synchronous read for non-React modules (search indexers, plain helpers).
// The React context (useLanguage) is the live source; this mirrors its persisted value.
export function currentLang(): Lang {
  try {
    return (typeof localStorage !== 'undefined' && localStorage.getItem('vox.lang') === 'la') ? 'la' : 'en';
  } catch {
    return 'en';
  }
}
