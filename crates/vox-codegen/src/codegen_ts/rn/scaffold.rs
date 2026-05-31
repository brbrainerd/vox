//! Expo project scaffold emitted by `vox build --target=mobile`.
//!
//! Produces the minimal set of build-system files that turn the emitted `.tsx`
//! sources into an installable mobile app via Expo Router + EAS Build:
//!
//! - `app.json` — Expo config (identifier, splash, version).
//! - `babel.config.js` — Babel preset for Expo.
//! - `metro.config.js` — Metro bundler config (with @vox/runtime-rn excluded from
//!                        per-platform tree-shaking).
//! - `eas.json` — EAS Build profiles (development / preview / production).
//! - `tsconfig.json` — TypeScript config tuned for RN+Expo.
//! - `package.json` — minimal dependency manifest, ready for `npm install`.
//! - `App.tsx` — root component re-exporting the first VUV component.
//!
//! Files are emitted unconditionally because the consuming build pipeline
//! (vox-cli/build.rs) skips writing any file that already exists in the output
//! directory (scaffold-once behavior). That keeps the CLI step idempotent.

use vox_compiler::hir::HirModule;

/// Emit the Expo project skeleton. When `has_routes` is true the package.json
/// `main` field points at `expo-router/entry` and the flat App.tsx is omitted
/// (Expo Router uses file-system routing under `app/`); when false the legacy
/// `expo/AppEntry.js` boot path is used with a generated App.tsx that mounts
/// the first declared VUV component.
pub fn emit_expo_scaffold(hir: &HirModule, has_routes: bool) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();

    let app_name = "vox-app";
    let bundle_id = "com.vox.app";

    out.push((
        "app.json".to_string(),
        emit_app_json(app_name, bundle_id, has_routes),
    ));
    out.push(("babel.config.js".to_string(), BABEL_CONFIG.to_string()));
    out.push(("metro.config.js".to_string(), METRO_CONFIG.to_string()));
    out.push(("eas.json".to_string(), EAS_JSON.to_string()));
    out.push(("tsconfig.json".to_string(), TSCONFIG_JSON.to_string()));
    out.push((
        "package.json".to_string(),
        emit_package_json(app_name, has_routes),
    ));

    // Only emit the flat App.tsx when there are no routes — Expo Router
    // owns the boot path otherwise via app/_layout.tsx + app/index.tsx.
    if !has_routes {
        if let Some(first) = hir.components.first() {
            out.push(("App.tsx".to_string(), emit_app_tsx(&first.name)));
        }
    }

    out
}

fn emit_app_json(name: &str, bundle_id: &str, has_routes: bool) -> String {
    // Use r## boundary so the embedded `#ffffff` color literal doesn't terminate the raw string.
    let plugins = if has_routes {
        "[\"expo-router\"]"
    } else {
        "[]"
    };
    let scheme_field = if has_routes {
        format!(",\n    \"scheme\": \"{name}\"")
    } else {
        String::new()
    };
    format!(
        r##"{{
  "expo": {{
    "name": "{name}",
    "slug": "{name}",
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
    "@vox/runtime-types": "0.6.0",
    "@vox/runtime-rn": "0.6.0"{router_dep}
  }},
  "devDependencies": {{
    "@types/react": "~18.3.0",
    "typescript": "~5.6.0"
  }}
}}
"#
    )
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
