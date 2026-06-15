//! Expo project scaffold emitted by `vox build --target=mobile`.
//!
//! Produces the minimal set of build-system files that turn the emitted `.tsx`
//! sources into an installable mobile app via Expo Router + EAS Build:
//!
//! - `app.json` — Expo config (identifier, splash, version).
//! - `babel.config.js` — Babel preset for Expo.
//! - `metro.config.js` — Metro bundler config (@vox/runtime-rn excluded from per-platform tree-shaking).
//! - `eas.json` — EAS Build profiles (development / preview / production).
//! - `tsconfig.json` — TypeScript config tuned for RN+Expo.
//! - `package.json` — minimal dependency manifest, ready for `npm install`.
//! - `App.tsx` — root component re-exporting the first VUV component.
//!
//! Files are emitted unconditionally because the consuming build pipeline
//! (vox-cli/build.rs) skips writing any file that already exists in the output
//! directory (scaffold-once behavior). That keeps the CLI step idempotent.

use vox_compiler::hir::HirModule;

/// SSOT for the generated `@vox/runtime-*` npm dependency pin emitted into
/// scaffolded React-Native projects. Bump in lockstep with the published runtime.
pub const VOX_RUNTIME_NPM_VERSION: &str = "0.6.0";

/// Emit the Expo project skeleton. When `has_routes` is true the package.json
/// `main` field points at `expo-router/entry` and the flat App.tsx is omitted
/// (Expo Router uses file-system routing under `app/`); when false the legacy
/// `expo/AppEntry.js` boot path is used with a generated App.tsx that mounts
/// the first declared VUV component.
///
/// `app_name` is the human-facing display name (`vox build --app-name`); the
/// Expo slug / scheme / npm package name are its slugified form. `app_id` is
/// the reverse-DNS identifier used for both the iOS bundle id and the Android
/// package (`vox build --app-id`). Defaults: `vox-app` / `com.vox.app`.
pub fn emit_expo_scaffold(
    hir: &HirModule,
    has_routes: bool,
    app_name: Option<&str>,
    app_id: Option<&str>,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();

    let app_name = app_name.unwrap_or("vox-app");
    let slug = slugify(app_name);
    let bundle_id = app_id.unwrap_or("com.vox.app");

    out.push((
        "app.json".to_string(),
        emit_app_json(app_name, &slug, bundle_id, has_routes),
    ));
    out.push(("babel.config.js".to_string(), BABEL_CONFIG.to_string()));
    out.push(("metro.config.js".to_string(), METRO_CONFIG.to_string()));
    out.push(("eas.json".to_string(), EAS_JSON.to_string()));
    out.push(("tsconfig.json".to_string(), TSCONFIG_JSON.to_string()));
    out.push((
        "package.json".to_string(),
        emit_package_json(&slug, has_routes),
    ));

    // Only emit the flat App.tsx when there are no routes — Expo Router
    // owns the boot path otherwise via app/_layout.tsx + app/index.tsx.
    if !has_routes && let Some(first) = hir.components.first() {
        out.push(("App.tsx".to_string(), emit_app_tsx(&first.name)));
    }

    out
}

/// Lowercase, alphanumeric-and-dash slug for Expo `slug`/`scheme` and the npm
/// package name (e.g. `"Vox Mental Tracker"` → `"vox-mental-tracker"`).
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = true; // suppress leading dashes
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn emit_app_json(name: &str, slug: &str, bundle_id: &str, has_routes: bool) -> String {
    // Use r## boundary so the embedded `#ffffff` color literal doesn't terminate the raw string.
    let plugins = if has_routes {
        "[\"expo-router\"]"
    } else {
        "[]"
    };
    let scheme_field = if has_routes {
        format!(",\n    \"scheme\": \"{slug}\"")
    } else {
        String::new()
    };
    format!(
        r##"{{
  "expo": {{
    "name": "{name}",
    "slug": "{slug}",
    "version": "0.1.0",
    "orientation": "portrait",
    "userInterfaceStyle": "automatic",
    "splash": {{
      "image": "./assets/splash.png",
      "resizeMode": "contain",
      "backgroundColor": "#ffffff"
    }},
    "ios": {{
      "supportsTablet": true,
      "bundleIdentifier": "{bundle_id}"
    }},
    "android": {{
      "package": "{bundle_id}"
    }},
    "plugins": {plugins}{scheme_field}
  }}
}}
"##
    )
}

const BABEL_CONFIG: &str = r#"module.exports = function (api) {
  api.cache(true);
  return {
    presets: ['babel-preset-expo']
  };
};
"#;

const METRO_CONFIG: &str = r#"const { getDefaultConfig } = require('expo/metro-config');

const config = getDefaultConfig(__dirname);

module.exports = config;
"#;

const EAS_JSON: &str = r#"{
  "cli": {
    "version": ">= 5.0.0"
  },
  "build": {
    "development": {
      "developmentClient": true,
      "distribution": "internal",
      "android": { "buildType": "apk" }
    },
    "preview": {
      "distribution": "internal",
      "android": { "buildType": "apk" }
    },
    "production": {
      "autoIncrement": true
    }
  },
  "submit": {
    "production": {}
  }
}
"#;

const TSCONFIG_JSON: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "lib": ["ES2022", "DOM"],
    "jsx": "react-jsx",
    "strict": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "noEmit": true,
    "isolatedModules": true,
    "types": ["react-native", "expo"]
  },
  "include": ["**/*.ts", "**/*.tsx"],
  "exclude": ["node_modules", "babel.config.js", "metro.config.js"]
}
"#;

fn emit_package_json(name: &str, has_routes: bool) -> String {
    // Expo Router owns the boot path when routes are declared; otherwise the
    // legacy AppEntry expects `App.tsx` at the project root.
    let main_field = if has_routes {
        "expo-router/entry"
    } else {
        "node_modules/expo/AppEntry.js"
    };
    let router_dep = if has_routes {
        ",\n    \"expo-router\": \"~4.0.0\",\n    \"expo-linking\": \"~7.0.0\",\n    \"expo-constants\": \"~17.0.0\",\n    \"react-native-screens\": \"~4.4.0\""
    } else {
        ""
    };
    format!(
        r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "private": true,
  "main": "{main_field}",
  "scripts": {{
    "start": "expo start",
    "android": "expo start --android",
    "ios": "expo start --ios",
    "build:preview": "eas build --profile preview --platform all",
    "build:production": "eas build --profile production --platform all"
  }},
  "dependencies": {{
    "expo": "^52.0.0",
    "expo-asset": "~11.0.5",
    "expo-status-bar": "~2.0.1",
    "react": "18.3.1",
    "react-native": "0.76.9",
    "react-native-safe-area-context": "4.12.0",
    "zod": "^3.23.8",
    "@vox/runtime-types": "{rt}",
    "@vox/runtime-rn": "{rt}"{router_dep}
  }},
  "devDependencies": {{
    "@types/react": "~18.3.0",
    "typescript": "~5.6.0"
  }}
}}
"#,
        rt = VOX_RUNTIME_NPM_VERSION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_module() -> HirModule {
        HirModule::default()
    }

    fn file<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
        &files
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("expected {name} in scaffold output"))
            .1
    }

    #[test]
    fn default_identity_is_vox_app() {
        let files = emit_expo_scaffold(&empty_module(), true, None, None);
        let app_json = file(&files, "app.json");
        assert!(app_json.contains("\"name\": \"vox-app\""));
        assert!(app_json.contains("\"bundleIdentifier\": \"com.vox.app\""));
        assert!(app_json.contains("\"package\": \"com.vox.app\""));
    }

    #[test]
    fn custom_identity_flows_into_app_json_and_package_json() {
        let files = emit_expo_scaffold(
            &empty_module(),
            true,
            Some("Vox Mental Tracker"),
            Some("com.vox.mentaltracker"),
        );
        let app_json = file(&files, "app.json");
        // Display name verbatim; slug + scheme are URL-safe slugs.
        assert!(app_json.contains("\"name\": \"Vox Mental Tracker\""));
        assert!(app_json.contains("\"slug\": \"vox-mental-tracker\""));
        assert!(app_json.contains("\"scheme\": \"vox-mental-tracker\""));
        assert!(app_json.contains("\"bundleIdentifier\": \"com.vox.mentaltracker\""));
        assert!(app_json.contains("\"package\": \"com.vox.mentaltracker\""));
        // npm package name must be the slug, never the display name.
        let pkg = file(&files, "package.json");
        assert!(pkg.contains("\"name\": \"vox-mental-tracker\""));
    }

    #[test]
    fn runtime_version_is_semver_triple() {
        let parts: Vec<&str> = VOX_RUNTIME_NPM_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "expected MAJOR.MINOR.PATCH");
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }

    #[test]
    fn slugify_handles_spaces_case_and_symbols() {
        assert_eq!(slugify("Vox Mental Tracker"), "vox-mental-tracker");
        assert_eq!(slugify("already-slugged"), "already-slugged");
        assert_eq!(slugify("Weird  __ Name!!"), "weird-name");
    }
}

fn emit_app_tsx(first_component: &str) -> String {
    format!(
        r#"// Auto-emitted root component. The first declared `component` in your Vox source
// is wired as the entry view. To customize routing or layout, edit this file and
// declare additional components.
import React from "react";
import {{ SafeAreaProvider }} from "react-native-safe-area-context";
import {{ StatusBar }} from "expo-status-bar";
import {{ {first_component} }} from "./{first_component}";

export default function App(): React.ReactElement {{
  return (
    <SafeAreaProvider>
      <StatusBar style="auto" />
      <{first_component} />
    </SafeAreaProvider>
  );
}}
"#
    )
}
