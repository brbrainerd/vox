# Build Android (React Native + Expo)

Tauri is desktop-only in Vox (ADR: scope-tauri-desktop-only); Android builds go
through the React Native + Expo target. The Expo project under `mobile/` is
**generated** from `src/main.vox` — never hand-edit it.

1. **Generate the Expo project**

   ```bash
   pnpm mobile:gen
   # = vox build src/main.vox --target=mobile -o mobile \
   #     --app-name "Vox Mental Tracker" --app-id com.vox.mentaltracker
   ```

2. **Run it**

   ```bash
   cd mobile
   npm install
   npx expo start            # dev server (Expo Go or dev client)
   npx expo run:android      # local debug build (needs Android SDK)
   ```

   Note: until the native `@vox/runtime-rn` module ships in your build,
   CI swaps in a JS shim (see `.github/workflows/mobile-eas-build.yml`) so the
   bundle resolves; runtime methods that need on-device Rust report
   `UnsupportedOnPlatform`.

3. **Installable APK via EAS** (cloud build, no local SDK needed)

   ```bash
   npx eas build --profile preview --platform android
   ```

   Requires an Expo account (`EXPO_TOKEN` in CI enables the automated job).

4. **Permissions**: mic/notification/camera entries are added at prebuild time
   by the `@vox/runtime-rn` Expo config plugin (feature-gated; description
   strings overridable via plugin props in `app.json`). No manual
   `AndroidManifest.xml` edits.

5. **Signing / Play**: EAS manages credentials by default; for local keystores,
   never commit secrets.
