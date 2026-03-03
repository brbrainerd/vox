use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

/// Type alias for the dependency graph map: (package, version) → [(dep_name, version_req, optional, features)]
type DepGraph = HashMap<(String, SemVer), Vec<(String, String, bool, Vec<String>)>>;

/// A parsed semantic version: major.minor.patch with optional pre-release.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl SemVer {
    /// Parse a version string like `"1.2.3"` or `"1.2.3-beta.1"`.
    pub fn parse(s: &str) -> Result<Self, ResolverError> {
        let s = s.trim().trim_start_matches('v');
        let (version_part, pre) = if let Some(idx) = s.find('-') {
            (&s[..idx], Some(s[idx + 1..].to_string()))
        } else {
            (s, None)
        };
        // Strip build metadata after +
        let version_part = version_part.split('+').next().unwrap_or(version_part);
        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(ResolverError::InvalidVersion(s.to_string()));
        }
        let major = parts[0]
            .parse()
            .map_err(|_| ResolverError::InvalidVersion(s.to_string()))?;
        let minor = if parts.len() > 1 {
            parts[1]
                .parse()
                .map_err(|_| ResolverError::InvalidVersion(s.to_string()))?
        } else {
            0
        };
        let patch = if parts.len() > 2 {
            parts[2]
                .parse()
                .map_err(|_| ResolverError::InvalidVersion(s.to_string()))?
        } else {
            0
        };
        Ok(Self {
            major,
            minor,
            patch,
            pre,
        })
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (Some(_), None) => Ordering::Less, // pre-release < release
                (None, Some(_)) => Ordering::Greater,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

/// A version requirement/range.
#[derive(Debug, Clone)]
pub enum VersionReq {
    /// `*` — any version
    Any,
    /// `^1.2.3` — compatible (Cargo-style default)
    Caret(SemVer),
    /// `~1.2.3` — approximately (allows patch bumps)
    Tilde(SemVer),
    /// `=1.2.3` — exact match
    Exact(SemVer),
    /// `>=1.2.3`
    Gte(SemVer),
    /// `>1.2.3`
    Gt(SemVer),
    /// `<=1.2.3`
    Lte(SemVer),
    /// `<1.2.3`
    Lt(SemVer),
    /// Intersection of multiple requirements, e.g. `>=1.0, <2.0`
    And(Vec<VersionReq>),
}

impl VersionReq {
    /// Parse a version requirement string.
    pub fn parse(s: &str) -> Result<Self, ResolverError> {
        let s = s.trim();
        if s == "*" {
            return Ok(Self::Any);
        }
        // Handle comma-separated compound requirements
        if s.contains(',') {
            let parts: Result<Vec<VersionReq>, _> =
                s.split(',').map(|p| VersionReq::parse(p.trim())).collect();
            return Ok(Self::And(parts?));
        }
        if let Some(rest) = s.strip_prefix("^") {
            return Ok(Self::Caret(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix("~") {
            return Ok(Self::Tilde(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix(">=") {
            return Ok(Self::Gte(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix('>') {
            return Ok(Self::Gt(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix("<=") {
            return Ok(Self::Lte(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix('<') {
            return Ok(Self::Lt(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix('=') {
            return Ok(Self::Exact(SemVer::parse(rest)?));
        }
        // Default: treat bare version as caret (Cargo convention)
        Ok(Self::Caret(SemVer::parse(s)?))
    }

    /// Check if a version matches this requirement.
    pub fn matches(&self, version: &SemVer) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(v) => version == v,
            Self::Caret(v) => {
                if v.major == 0 {
                    if v.minor == 0 {
                        // ^0.0.x — only exact patch
                        version.major == 0 && version.minor == 0 && version.patch == v.patch
                    } else {
                        // ^0.y.z — same minor
                        version.major == 0 && version.minor == v.minor && version.patch >= v.patch
                    }
                } else {
                    // ^x.y.z — same major, >= minor.patch
                    version.major == v.major && version >= v
                }
            }
            Self::Tilde(v) => {
                // ~x.y.z — same major.minor, >= patch
                version.major == v.major && version.minor == v.minor && version.patch >= v.patch
            }
            Self::Gte(v) => version >= v,
            Self::Gt(v) => version > v,
            Self::Lte(v) => version <= v,
            Self::Lt(v) => version < v,
            Self::And(reqs) => reqs.iter().all(|r| r.matches(version)),
        }
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => write!(f, "*"),
            Self::Caret(v) => write!(f, "^{v}"),
            Self::Tilde(v) => write!(f, "~{v}"),
            Self::Exact(v) => write!(f, "={v}"),
            Self::Gte(v) => write!(f, ">={v}"),
            Self::Gt(v) => write!(f, ">{v}"),
            Self::Lte(v) => write!(f, "<={v}"),
            Self::Lt(v) => write!(f, "<{v}"),
            Self::And(reqs) => {
                for (i, r) in reqs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{r}")?;
                }
                Ok(())
            }
        }
    }
}

/// A resolved dependency with its exact version.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub version: SemVer,
    pub hash: String,
    pub features: Vec<String>,
}

/// Package metadata for the registry/available packages.
#[derive(Debug, Clone)]
pub struct AvailablePackage {
    pub name: String,
    pub versions: Vec<SemVer>,
    pub deps: BTreeMap<String, (SemVer, Vec<(String, String)>)>, // version -> [(dep_name, version_req)]
}

/// The dependency resolver.
pub struct Resolver {
    /// Available packages in the registry (name -> available versions).
    available: HashMap<String, Vec<SemVer>>,
    /// Dependencies for each package@version: (dep_name, version_req_string, optional, features).
    dep_graph: DepGraph,
    /// Feature map: (package, version) -> { feature_name: [implied_features_or_deps] }
    feature_graph: HashMap<(String, SemVer), std::collections::BTreeMap<String, Vec<String>>>,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            available: HashMap::new(),
            dep_graph: HashMap::new(),
            feature_graph: HashMap::new(),
        }
    }

    /// Register an available package version.
    pub fn add_available(&mut self, name: &str, version: SemVer) {
        self.available
            .entry(name.to_string())
            .or_default()
            .push(version);
    }

    /// Register dependencies for a specific package@version.
    pub fn add_deps(
        &mut self,
        name: &str,
        version: SemVer,
        deps: Vec<(String, String, bool, Vec<String>)>,
    ) {
        self.dep_graph.insert((name.to_string(), version), deps);
    }

    /// Register features for a specific package@version.
    pub fn add_features(
        &mut self,
        name: &str,
        version: SemVer,
        features: std::collections::BTreeMap<String, Vec<String>>,
    ) {
        self.feature_graph
            .insert((name.to_string(), version), features);
    }

    /// Resolve dependencies for a root set of requirements.
    /// Returns a flat list of resolved packages or an error.
    pub fn resolve(
        &self,
        root_deps: &[(String, String, Vec<String>)],
    ) -> Result<Vec<ResolvedDep>, ResolverError> {
        let mut resolved: BTreeMap<String, SemVer> = BTreeMap::new();
        // Packge -> activated features
        let mut active_features: HashMap<String, std::collections::HashSet<String>> =
            HashMap::new();

        let mut queue: VecDeque<(String, String, Vec<String>)> = root_deps
            .iter()
            .map(|(n, v, f)| (n.clone(), v.clone(), f.clone()))
            .collect();

        while let Some((name, version_req_str, requested_features)) = queue.pop_front() {
            let req = VersionReq::parse(&version_req_str)?;

            let is_new = !resolved.contains_key(&name);
            let selected = if let Some(existing) = resolved.get(&name) {
                if !req.matches(existing) {
                    return Err(ResolverError::Conflict(
                        name.clone(),
                        existing.to_string(),
                        version_req_str.clone(),
                    ));
                }
                existing.clone()
            } else {
                let versions = self
                    .available
                    .get(&name)
                    .ok_or_else(|| ResolverError::PackageNotFound(name.clone()))?;

                let mut candidates: Vec<&SemVer> =
                    versions.iter().filter(|v| req.matches(v)).collect();
                candidates.sort();
                candidates.reverse(); // highest first

                let selected = candidates.first().ok_or_else(|| {
                    ResolverError::NoMatchingVersion(name.clone(), version_req_str.clone())
                })?;

                resolved.insert(name.clone(), (*selected).clone());
                (*selected).clone()
            };

            let key = (name.clone(), selected.clone());
            let pkg_features = self.feature_graph.get(&key);

            let mut newly_activated = Vec::new();
            if is_new {
                let active = active_features.entry(name.clone()).or_default();
                if active.insert("default".to_string()) {
                    newly_activated.push("default".to_string());
                }
            }

            for f in requested_features {
                let active = active_features.entry(name.clone()).or_default();
                if active.insert(f.clone()) {
                    newly_activated.push(f);
                }
            }

            let mut final_new_features = Vec::new();
            let mut feature_queue = newly_activated.clone();
            while let Some(feat) = feature_queue.pop() {
                final_new_features.push(feat.clone());
                if let Some(feature_map) = pkg_features {
                    if let Some(implied) = feature_map.get(&feat) {
                        for imp in implied {
                            let active = active_features.entry(name.clone()).or_default();
                            if active.insert(imp.clone()) {
                                feature_queue.push(imp.clone());
                            }
                        }
                    }
                }
            }

            if is_new || !final_new_features.is_empty() {
                if let Some(deps) = self.dep_graph.get(&key) {
                    for (dep_name, dep_req, optional, dep_feat) in deps {
                        let mut should_add = !*optional;
                        if *optional {
                            let active = active_features.entry(name.clone()).or_default();
                            if active.contains(dep_name)
                                || active.contains(&format!("dep:{}", dep_name))
                            {
                                should_add = true;
                            }
                        }

                        if should_add {
                            let dep_active = active_features.entry(dep_name.clone()).or_default();
                            let mut has_new_dep_features = false;
                            for df in dep_feat {
                                if !dep_active.contains(df) {
                                    has_new_dep_features = true;
                                }
                            }
                            let dep_is_new = !resolved.contains_key(dep_name);

                            if dep_is_new || has_new_dep_features {
                                queue.push_back((
                                    dep_name.clone(),
                                    dep_req.clone(),
                                    dep_feat.clone(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        Ok(resolved
            .into_iter()
            .map(|(name, version)| {
                let mut features: Vec<String> = active_features
                    .get(&name)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                features.sort();
                ResolvedDep {
                    name,
                    version,
                    hash: String::new(),
                    features,
                }
            })
            .collect())
    }
}

#[derive(Debug)]
pub enum ResolverError {
    InvalidVersion(String),
    InvalidVersionReq(String),
    PackageNotFound(String),
    NoMatchingVersion(String, String),
    Conflict(String, String, String),
    CycleDetected(Vec<String>),
}

impl fmt::Display for ResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVersion(v) => write!(f, "Invalid version: {v}"),
            Self::InvalidVersionReq(r) => write!(f, "Invalid version requirement: {r}"),
            Self::PackageNotFound(n) => write!(f, "Package not found: {n}"),
            Self::NoMatchingVersion(n, r) => {
                write!(f, "No version of {n} matches requirement {r}")
            }
            Self::Conflict(n, v1, v2) => {
                write!(f, "Version conflict for {n}: {v1} vs {v2}")
            }
            Self::CycleDetected(path) => {
                write!(f, "Dependency cycle detected: {}", path.join(" -> "))
            }
        }
    }
}

impl std::error::Error for ResolverError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre.is_none());
    }

    #[test]
    fn test_parse_semver_prerelease() {
        let v = SemVer::parse("1.0.0-beta.1").unwrap();
        assert_eq!(v.pre, Some("beta.1".to_string()));
    }

    #[test]
    fn test_parse_semver_short() {
        let v = SemVer::parse("2").unwrap();
        assert_eq!(
            v,
            SemVer {
                major: 2,
                minor: 0,
                patch: 0,
                pre: None
            }
        );
    }

    #[test]
    fn test_semver_ordering() {
        let v1 = SemVer::parse("1.0.0").unwrap();
        let v2 = SemVer::parse("1.0.1").unwrap();
        let v3 = SemVer::parse("1.1.0").unwrap();
        let v4 = SemVer::parse("2.0.0").unwrap();
        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
    }

    #[test]
    fn test_prerelease_less_than_release() {
        let pre = SemVer::parse("1.0.0-alpha").unwrap();
        let rel = SemVer::parse("1.0.0").unwrap();
        assert!(pre < rel);
    }

    #[test]
    fn test_caret_req() {
        let req = VersionReq::parse("^1.2.0").unwrap();
        assert!(req.matches(&SemVer::parse("1.2.0").unwrap()));
        assert!(req.matches(&SemVer::parse("1.3.0").unwrap()));
        assert!(req.matches(&SemVer::parse("1.99.99").unwrap()));
        assert!(!req.matches(&SemVer::parse("2.0.0").unwrap()));
        assert!(!req.matches(&SemVer::parse("1.1.0").unwrap()));
    }

    #[test]
    fn test_caret_zero_major() {
        let req = VersionReq::parse("^0.2.0").unwrap();
        assert!(req.matches(&SemVer::parse("0.2.0").unwrap()));
        assert!(req.matches(&SemVer::parse("0.2.5").unwrap()));
        assert!(!req.matches(&SemVer::parse("0.3.0").unwrap()));
    }

    #[test]
    fn test_tilde_req() {
        let req = VersionReq::parse("~1.2.0").unwrap();
        assert!(req.matches(&SemVer::parse("1.2.0").unwrap()));
        assert!(req.matches(&SemVer::parse("1.2.9").unwrap()));
        assert!(!req.matches(&SemVer::parse("1.3.0").unwrap()));
    }

    #[test]
    fn test_compound_req() {
        let req = VersionReq::parse(">=1.0.0, <2.0.0").unwrap();
        assert!(req.matches(&SemVer::parse("1.0.0").unwrap()));
        assert!(req.matches(&SemVer::parse("1.5.0").unwrap()));
        assert!(!req.matches(&SemVer::parse("2.0.0").unwrap()));
        assert!(!req.matches(&SemVer::parse("0.9.0").unwrap()));
    }

    #[test]
    fn test_any_req() {
        let req = VersionReq::parse("*").unwrap();
        assert!(req.matches(&SemVer::parse("0.0.1").unwrap()));
        assert!(req.matches(&SemVer::parse("999.0.0").unwrap()));
    }

    #[test]
    fn test_resolver_basic() {
        let mut resolver = Resolver::new();
        resolver.add_available("foo", SemVer::parse("1.0.0").unwrap());
        resolver.add_available("foo", SemVer::parse("1.1.0").unwrap());
        resolver.add_available("foo", SemVer::parse("2.0.0").unwrap());

        let result = resolver
            .resolve(&[("foo".into(), "^1.0".into(), vec![])])
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "foo");
        assert_eq!(result[0].version, SemVer::parse("1.1.0").unwrap());
    }

    #[test]
    fn test_resolver_transitive() {
        let mut resolver = Resolver::new();
        resolver.add_available("app", SemVer::parse("1.0.0").unwrap());
        resolver.add_available("core", SemVer::parse("2.0.0").unwrap());
        resolver.add_available("core", SemVer::parse("2.1.0").unwrap());
        resolver.add_available("utils", SemVer::parse("0.5.0").unwrap());

        resolver.add_deps(
            "app",
            SemVer::parse("1.0.0").unwrap(),
            vec![("core".into(), "^2.0".into(), false, vec![])],
        );
        resolver.add_deps(
            "core",
            SemVer::parse("2.1.0").unwrap(),
            vec![("utils".into(), "^0.5".into(), false, vec![])],
        );

        let result = resolver
            .resolve(&[("app".into(), "^1.0".into(), vec![])])
            .unwrap();

        assert_eq!(result.len(), 3);
        let names: Vec<&str> = result.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"app"));
        assert!(names.contains(&"core"));
        assert!(names.contains(&"utils"));
    }

    #[test]
    fn test_resolver_not_found() {
        let resolver = Resolver::new();
        let result = resolver.resolve(&[("missing".into(), "^1.0".into(), vec![])]);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolver_no_matching_version() {
        let mut resolver = Resolver::new();
        resolver.add_available("foo", SemVer::parse("1.0.0").unwrap());

        let result = resolver.resolve(&[("foo".into(), "^2.0".into(), vec![])]);
        assert!(result.is_err());
    }
}
