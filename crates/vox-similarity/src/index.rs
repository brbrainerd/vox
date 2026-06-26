//! LSH band index over minhash signatures: cheap near-neighbor candidate
//! generation, plus jaccard-confirmed clustering and one-vs-many overlap.

use std::collections::{BTreeSet, HashMap};

use crate::fragment::Fragment;
use crate::signature::{Signature, jaccard_estimate};

/// A confirmed near-duplicate match for a query fragment.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub index: usize,
    pub jaccard: f32,
}

/// A group of mutually-similar fragments.
#[derive(Debug, Clone, PartialEq)]
pub struct Cluster {
    pub members: Vec<usize>,
}

/// Banded LSH index. `bands * rows` must equal the minhash length used by inserted
/// fragments for full banding; shorter signatures are clamped.
pub struct LshIndex {
    fragments: Vec<Fragment>,
    bands: usize,
    rows: usize,
    buckets: HashMap<(usize, u64), Vec<usize>>,
}

impl LshIndex {
    pub fn new(bands: usize, rows: usize) -> Self {
        Self {
            fragments: Vec::new(),
            bands,
            rows,
            buckets: HashMap::new(),
        }
    }

    fn band_keys(&self, sig: &Signature) -> Vec<(usize, u64)> {
        let mut keys = Vec::new();
        for b in 0..self.bands {
            let start = b * self.rows;
            let end = (start + self.rows).min(sig.minhash.len());
            if start >= end {
                break;
            }
            let mut hasher = blake3::Hasher::new();
            for &v in &sig.minhash[start..end] {
                hasher.update(&v.to_le_bytes());
            }
            let h = hasher.finalize();
            let key = u64::from_le_bytes(h.as_bytes()[0..8].try_into().unwrap());
            keys.push((b, key));
        }
        keys
    }

    pub fn insert(&mut self, fragment: Fragment) -> usize {
        let idx = self.fragments.len();
        for key in self.band_keys(&fragment.signature) {
            self.buckets.entry(key).or_default().push(idx);
        }
        self.fragments.push(fragment);
        idx
    }

    pub fn len(&self) -> usize {
        self.fragments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }

    pub fn fragment(&self, idx: usize) -> &Fragment {
        &self.fragments[idx]
    }

    fn near_indices(&self, sig: &Signature) -> Vec<usize> {
        let mut set = BTreeSet::new();
        for key in self.band_keys(sig) {
            if let Some(v) = self.buckets.get(&key) {
                for &i in v {
                    set.insert(i);
                }
            }
        }
        set.into_iter().collect()
    }

    /// Confirmed matches for an external query fragment. Excludes any indexed
    /// fragment that shares the query's content hash AND source_ref (self).
    pub fn overlap(&self, query: &Fragment, min_jaccard: f32) -> Vec<Match> {
        let mut out = Vec::new();
        for i in self.near_indices(&query.signature) {
            let f = &self.fragments[i];
            if f.content_hash == query.content_hash && f.source_ref == query.source_ref {
                continue;
            }
            let j = jaccard_estimate(&query.signature.minhash, &f.signature.minhash);
            if j >= min_jaccard {
                out.push(Match {
                    index: i,
                    jaccard: j,
                });
            }
        }
        out.sort_by(|a, b| {
            b.jaccard
                .partial_cmp(&a.jaccard)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    fn neighbors_of(&self, idx: usize, min_jaccard: f32) -> Vec<usize> {
        let q = &self.fragments[idx];
        let mut out = Vec::new();
        for i in self.near_indices(&q.signature) {
            if i == idx {
                continue;
            }
            let j = jaccard_estimate(&q.signature.minhash, &self.fragments[i].signature.minhash);
            if j >= min_jaccard {
                out.push(i);
            }
        }
        out
    }

    /// Group indexed fragments into clusters via union-find over confirmed
    /// neighbor pairs. Returns only clusters with `>= min_members`.
    pub fn cluster(&self, min_members: usize, min_jaccard: f32) -> Vec<Cluster> {
        let n = self.fragments.len();
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while parent[r] != r {
                r = parent[r];
            }
            let mut c = x;
            while parent[c] != c {
                let next = parent[c];
                parent[c] = r;
                c = next;
            }
            r
        }

        for i in 0..n {
            for m in self.neighbors_of(i, min_jaccard) {
                let a = find(&mut parent, i);
                let b = find(&mut parent, m);
                if a != b {
                    parent[a] = b;
                }
            }
        }

        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let r = find(&mut parent, i);
            groups.entry(r).or_default().push(i);
        }

        let mut clusters: Vec<Cluster> = groups
            .into_values()
            .filter(|g| g.len() >= min_members)
            .map(|mut members| {
                members.sort_unstable();
                Cluster { members }
            })
            .collect();
        clusters.sort_by(|a, b| a.members.cmp(&b.members));
        clusters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::{Fragment, FragmentKind};

    fn frag(id: &str, text: &str, src: &str) -> Fragment {
        Fragment::new(id, FragmentKind::Code, text, src, 3, 64)
    }

    #[test]
    fn cluster_groups_identical_fragments() {
        let mut idx = LshIndex::new(16, 4);
        idx.insert(frag("a", "let total = price * quantity + tax", "a.vox:1"));
        idx.insert(frag("b", "let total = price * quantity + tax", "b.vox:9"));
        idx.insert(frag("c", "print hello world unrelated entirely", "c.vox:3"));
        let clusters = idx.cluster(2, 0.7);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].members, vec![0, 1]);
    }

    #[test]
    fn overlap_finds_similar_query() {
        let mut idx = LshIndex::new(16, 4);
        idx.insert(frag("a", "let total = price * quantity + tax", "a.vox:1"));
        let q = frag("q", "let total = price * quantity + tax", "q.vox:1");
        let matches = idx.overlap(&q, 0.7);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].index, 0);
        assert!(matches[0].jaccard >= 0.9);
    }

    #[test]
    fn no_clusters_when_all_unique() {
        let mut idx = LshIndex::new(16, 4);
        idx.insert(frag("a", "alpha beta gamma delta epsilon", "a:1"));
        idx.insert(frag("b", "one two three four five six", "b:1"));
        assert!(idx.cluster(2, 0.7).is_empty());
    }
}
