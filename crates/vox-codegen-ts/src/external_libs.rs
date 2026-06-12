//! SSOT: external React / React-Native component libraries that Vox knows how
//! to support when they are pulled in via `import react … from "<pkg>"`.
//!
//! For each known package this records the styling runtime, any **required CSS
//! file imports** (which the emitter injects automatically — e.g. Mantine), the
//! mandatory/optional top-level **provider** (surfaced as setup guidance), the
//! peer dependencies, and which target(s) the library is valid for (web vs RN).
//!
//! All data here is anchored to the verified package facts in
//! `docs/src/architecture/external-frontend-interop-phase5-component-interop-subspec-2026.md §4`.
//! It is a single Rust table rather than a contract YAML to stay dependency-free
//! and avoid an orphaned contract; promote to `contracts/frontend/` if a second
//! consumer appears.

/// Build target a library is valid for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibTarget {
    /// Web (React DOM) only — not importable on the RN target.
    Web,
    /// React Native only.
    Rn,
    /// Valid on both targets.
    Both,
}

/// Styling runtime a library ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Styling {
    /// Runtime CSS-in-JS via Emotion (MUI, Chakra).
    Emotion,
    /// Runtime CSS-in-JS, library-bundled (antd `@ant-design/cssinjs`).
    CssInJs,
    /// Ships real CSS files the consumer MUST import (Mantine).
    CssFile,
    /// Utility classes scanned at build time (NativeWind / Tailwind).
    Tailwind,
    /// Unstyled / headless (Radix, Headless UI, React Aria).
    None,
}

/// One known external component library.
#[derive(Debug, Clone, Copy)]
pub struct ExternalLib {
    /// Bare npm package specifier (e.g. `@mui/material`).
    pub package: &'static str,
    /// Styling runtime.
    pub styling: Styling,
    /// CSS files the emitter must inject (`import "<path>";`). Usually empty.
    pub css_imports: &'static [&'static str],
    /// Top-level provider component name, if any (e.g. `MantineProvider`).
    pub provider: Option<&'static str>,
    /// Whether the provider is mandatory for components to render.
    pub provider_mandatory: bool,
    /// Peer dependencies the consumer must install (advisory).
    pub peers: &'static [&'static str],
    /// Target(s) the library is valid for.
    pub target: LibTarget,
}

/// SSOT table. Seeded from verified primary-source research (sub-spec §4).
pub const LIBRARIES: &[ExternalLib] = &[
    ExternalLib {
        package: "@mui/material",
        styling: Styling::Emotion,
        css_imports: &[],
        provider: Some("ThemeProvider"),
        provider_mandatory: false,
        peers: &["@emotion/react", "@emotion/styled"],
        target: LibTarget::Web,
    },
    ExternalLib {
        package: "@chakra-ui/react",
        styling: Styling::Emotion,
        css_imports: &[],
        provider: Some("ChakraProvider"),
        provider_mandatory: true,
        peers: &["@emotion/react"],
        target: LibTarget::Web,
    },
    ExternalLib {
        package: "@mantine/core",
        styling: Styling::CssFile,
        css_imports: &["@mantine/core/styles.css"],
        provider: Some("MantineProvider"),
        provider_mandatory: true,
        peers: &["@mantine/hooks"],
        target: LibTarget::Web,
    },
    ExternalLib {
        package: "antd",
        styling: Styling::CssInJs,
        css_imports: &[],
        provider: Some("ConfigProvider"),
        provider_mandatory: false,
        peers: &[],
        target: LibTarget::Web,
    },
    ExternalLib {
        package: "@radix-ui/react-dialog",
        styling: Styling::None,
        css_imports: &[],
        provider: None,
        provider_mandatory: false,
        peers: &[],
        target: LibTarget::Web,
    },
    ExternalLib {
        package: "radix-ui",
        styling: Styling::None,
        css_imports: &[],
        provider: None,
        provider_mandatory: false,
        peers: &[],
        target: LibTarget::Web,
    },
    ExternalLib {
        package: "@headlessui/react",
        styling: Styling::None,
        css_imports: &[],
        provider: None,
        provider_mandatory: false,
        peers: &[],
        target: LibTarget::Web,
    },
    ExternalLib {
        package: "react-aria-components",
        styling: Styling::None,
        css_imports: &[],
        provider: Some("I18nProvider"),
        provider_mandatory: false,
        peers: &[],
        target: LibTarget::Web,
    },
    ExternalLib {
        package: "react-native-paper",
        styling: Styling::None,
        css_imports: &[],
        provider: Some("PaperProvider"),
        provider_mandatory: true,
        peers: &["react-native-safe-area-context"],
        target: LibTarget::Rn,
    },
    ExternalLib {
        package: "tamagui",
        styling: Styling::None,
        css_imports: &[],
        provider: Some("TamaguiProvider"),
        provider_mandatory: true,
        peers: &[],
        target: LibTarget::Rn,
    },
    ExternalLib {
        package: "nativewind",
        styling: Styling::Tailwind,
        css_imports: &[],
        provider: None,
        provider_mandatory: false,
        peers: &["tailwindcss"],
        target: LibTarget::Rn,
    },
];

/// Extract the bare npm package from a module specifier:
/// `@mui/material/Button` → `@mui/material`; `react-aria/useButton` → `react-aria`.
/// Returns `None` for relative specifiers (`./Foo`, `../Foo.tsx`).
#[must_use]
pub fn bare_package(spec: &str) -> Option<&str> {
    if spec.starts_with('.') || spec.starts_with('/') {
        return None;
    }
    if let Some(rest) = spec.strip_prefix('@') {
        // Scoped: keep `@scope/name`.
        let mut it = rest.splitn(3, '/');
        let scope = it.next()?;
        let name = it.next()?;
        // SAFETY: scope+name are a prefix of `spec`; reconstruct a borrowed slice.
        let len = 1 + scope.len() + 1 + name.len();
        Some(&spec[..len.min(spec.len())])
    } else {
        Some(spec.split('/').next().unwrap_or(spec))
    }
}

/// Look up a known library by module specifier (resolves the bare package first).
#[must_use]
pub fn lookup(spec: &str) -> Option<&'static ExternalLib> {
    let bare = bare_package(spec)?;
    LIBRARIES.iter().find(|l| l.package == bare)
}

/// Whether a library is usable on the given target string (`"web"` or `"rn"`).
#[must_use]
pub fn valid_for_target(lib: &ExternalLib, target_is_rn: bool) -> bool {
    match lib.target {
        LibTarget::Both => true,
        LibTarget::Web => !target_is_rn,
        LibTarget::Rn => target_is_rn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_package_handles_scoped_subpath_and_relative() {
        assert_eq!(bare_package("@mui/material/Button"), Some("@mui/material"));
        assert_eq!(bare_package("@mui/material"), Some("@mui/material"));
        assert_eq!(bare_package("react-aria/useButton"), Some("react-aria"));
        assert_eq!(
            bare_package("react-native-paper"),
            Some("react-native-paper")
        );
        assert_eq!(bare_package("./Foo.tsx"), None);
        assert_eq!(bare_package("../ui/Bar"), None);
    }

    #[test]
    fn lookup_resolves_known_libraries() {
        assert_eq!(lookup("@mantine/core").unwrap().styling, Styling::CssFile);
        assert_eq!(
            lookup("@mui/material/Button").unwrap().package,
            "@mui/material"
        );
        assert!(lookup("@radix-ui/react-dialog").unwrap().provider.is_none());
        assert!(lookup("totally-unknown-pkg").is_none());
        assert!(lookup("./local").is_none());
    }

    #[test]
    fn target_validity() {
        let mantine = lookup("@mantine/core").unwrap();
        assert!(valid_for_target(mantine, false)); // web
        assert!(!valid_for_target(mantine, true)); // not rn
        let paper = lookup("react-native-paper").unwrap();
        assert!(valid_for_target(paper, true));
        assert!(!valid_for_target(paper, false));
    }
}
