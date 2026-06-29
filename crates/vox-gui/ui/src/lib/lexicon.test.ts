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
    expect(LEXICON.mens.la).toBeUndefined();
    expect(pick(LEXICON.mens, 'la')).toBe('Mens');
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
