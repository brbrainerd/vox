use serde::{Deserialize, Serialize};
use std::fmt;

/// Enumerates all artifact kinds that VoxPM can manage as packages.
/// This is the core differentiator: one PM for all Vox artifact types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum PackageKind {
    /// Traditional code library.
    #[default]
    Library,
    /// Runnable application.
    Application,
    /// Reusable AI skill (e.g. text summarizer).
    Skill,
    /// AI agent definition.
    Agent,
    /// Workflow template/definition.
    Workflow,
    /// Code snippet.
    Snippet,
    /// UI component.
    Component,
}

impl PackageKind {
    /// Parse from a string, using case-insensitive matching.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "library" | "lib" => Some(Self::Library),
            "application" | "app" | "bin" => Some(Self::Application),
            "skill" => Some(Self::Skill),
            "agent" => Some(Self::Agent),
            "workflow" => Some(Self::Workflow),
            "snippet" => Some(Self::Snippet),
            "component" => Some(Self::Component),
            _ => None,
        }
    }

    /// The namespace used in the component registry for this kind.
    pub fn namespace(&self) -> &'static str {
        match self {
            Self::Library => "libraries",
            Self::Application => "applications",
            Self::Skill => "skills",
            Self::Agent => "agents",
            Self::Workflow => "workflows",
            Self::Snippet => "snippets",
            Self::Component => "components",
        }
    }

    /// Whether this kind should be installable as a dependency.
    pub fn is_dependency_eligible(&self) -> bool {
        matches!(
            self,
            Self::Library | Self::Skill | Self::Agent | Self::Workflow | Self::Component
        )
    }
}

impl fmt::Display for PackageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Library => "library",
            Self::Application => "application",
            Self::Skill => "skill",
            Self::Agent => "agent",
            Self::Workflow => "workflow",
            Self::Snippet => "snippet",
            Self::Component => "component",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str_loose() {
        assert_eq!(
            PackageKind::from_str_loose("library"),
            Some(PackageKind::Library)
        );
        assert_eq!(
            PackageKind::from_str_loose("lib"),
            Some(PackageKind::Library)
        );
        assert_eq!(
            PackageKind::from_str_loose("App"),
            Some(PackageKind::Application)
        );
        assert_eq!(
            PackageKind::from_str_loose("SKILL"),
            Some(PackageKind::Skill)
        );
        assert_eq!(
            PackageKind::from_str_loose("agent"),
            Some(PackageKind::Agent)
        );
        assert_eq!(
            PackageKind::from_str_loose("workflow"),
            Some(PackageKind::Workflow)
        );
        assert_eq!(
            PackageKind::from_str_loose("snippet"),
            Some(PackageKind::Snippet)
        );
        assert_eq!(
            PackageKind::from_str_loose("component"),
            Some(PackageKind::Component)
        );
        assert_eq!(PackageKind::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(PackageKind::Library.to_string(), "library");
        assert_eq!(PackageKind::Agent.to_string(), "agent");
    }

    #[test]
    fn test_namespace() {
        assert_eq!(PackageKind::Skill.namespace(), "skills");
        assert_eq!(PackageKind::Workflow.namespace(), "workflows");
    }

    #[test]
    fn test_dependency_eligible() {
        assert!(PackageKind::Library.is_dependency_eligible());
        assert!(PackageKind::Skill.is_dependency_eligible());
        assert!(!PackageKind::Application.is_dependency_eligible());
        assert!(!PackageKind::Snippet.is_dependency_eligible());
    }

    #[test]
    fn test_serde_roundtrip() {
        let kind = PackageKind::Agent;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, r#""agent""#);
        let parsed: PackageKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, kind);
    }
}
