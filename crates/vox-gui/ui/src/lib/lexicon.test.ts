// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { LEXICON, pick, labelFor } from './lexicon';

describe('lexicon', () => {
  it('English mode returns en', () => {
    expect(pick(LEXICON.mercatus, 'en')).toBe('Market');
  });
  it('Latin mode returns la when present', () => {
    expect(pick(LEXICON.mercatus, 'la')).toBe('Mercatus');
  });
  it('proper noun has no la and stays en in Latin mode', () => {
    expect(LEXICON['set-orchestrator'].la).toBeUndefined();
    expect(pick(LEXICON['set-orchestrator'], 'la')).toBe('Orchestrator');
  });
  it('de-Latinizes compute surface labels (Bundle 1 + Amendment A)', () => {
    expect(pick(LEXICON.oratio, 'en')).toBe('Voice');
    expect(pick(LEXICON.mens, 'en')).toBe('Training');
    expect(pick(LEXICON.populi, 'en')).toBe('Nodes');
  });
  it('labels the promoted Review group and the Discovery surface', () => {
    expect(pick(LEXICON['nav:runs'], 'en')).toBe('Review');
    expect(pick(LEXICON.activity, 'en')).toBe('Discovery');
  });
  it('labelFor returns the id for unknown entries', () => {
    expect(labelFor('nope', 'la')).toBe('nope');
  });
  it('resolves the knowledge/scientia collision', () => {
    expect(pick(LEXICON.knowledge, 'la')).toBe('Scientia');
    expect(pick(LEXICON.scientia, 'en')).toBe('Findings');
  });
  it('covers all 41 navigable viewKeys', () => {
    const viewKeys = ['activity','agents','approvals','archive-panel','browser','catalog','chat','claims','commands','compute','console','coverage','dashboard','discovery-inbox','discovery-review','flow','gamify','harness','knowledge','matrix','memory','mens','mercatus','mesh','models','needs-you','oratio','policies','populi','publications','repository','research','review','runs','scientia','search','settings','skills','sub-agents','vox-search','workspace'];
    for (const k of viewKeys) expect(LEXICON[k], `missing lexicon entry: ${k}`).toBeTruthy();
  });
});
