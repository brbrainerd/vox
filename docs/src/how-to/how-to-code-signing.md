---
title: "How to Configure Code Signing for Releases"
description: "Step-by-step guide to acquiring code signing certificates and configuring GitHub Actions to bypass Gatekeeper and SmartScreen."
category: "How-To Guides"

schema_type: "HowTo"
keywords: ["code signing", "Tauri code signing", "Apple Developer ID", "Windows EV certificate", "SmartScreen bypass", "Gatekeeper bypass"]
---
# How to Configure Code Signing for Releases

To distribute the `vox-gui` desktop application (or any user-generated Tauri application) without triggering severe security warnings on user machines, you must digitally sign your executables.

This guide covers the necessary steps to acquire the correct certificates for macOS and Windows, integrate them into GitHub Actions, and the free/alternative methods if you choose not to pay.

## macOS: Bypassing Apple Gatekeeper

Apple's Gatekeeper explicitly blocks unsigned applications from running. To bypass this, the app must be signed with a **Developer ID Application** certificate and **Notarized** by Apple's servers.

### The Official Method (Paid)
**Cost:** $99/year (Apple Developer Program)

1. **Enroll:** Join the [Apple Developer Program](https://developer.apple.com/programs/).
2. **Create Certificate:** Generate a **Developer ID Application** certificate in your Apple Developer account.
3. **Export `.p12`:** Download the certificate, double-click to install it into your Mac's Keychain Access. Then, find it in Keychain, right-click, select "Export", and save it as a `.p12` file (you will be prompted to create a password).
4. **Base64 Encode:** Run `openssl base64 -in your-cert.p12 -out cert-base64.txt` in your terminal.
5. **App Store Connect API Key:** Generate an API Key in App Store Connect with "Developer" access to use for automated notarization.
6. **Add to GitHub Secrets:** Add the following to your repository secrets:
   - `APPLE_CERTIFICATE`: The contents of `cert-base64.txt`
   - `APPLE_CERTIFICATE_PASSWORD`: The password you created during export
   - `APPLE_API_ISSUER`: The Issuer ID from App Store Connect
   - `APPLE_API_KEY`: The `.p8` API key content

### The Alternative Method (Free)
If you do not purchase an Apple Developer account, you cannot notarize the app. Users who download the app will receive an "App is damaged and cannot be opened" or "Unidentified Developer" error.
**User workaround:** Users must bypass Gatekeeper manually by running `xattr -cr /Applications/Vox.app` in their terminal, or by right-clicking the app and selecting **Open**.

---

## Windows: Bypassing Microsoft SmartScreen

Windows uses SmartScreen to warn users about unrecognized executables. Since 2023, industry regulations require **all** code signing certificates to be stored on physical hardware (USB tokens) or Cloud HSMs (Hardware Security Modules).

### The Official Method (Cloud-Native)
**Recommended:** Microsoft Azure Trusted Signing
**Cost:** Varies based on Azure consumption, typically highly cost-effective for CI/CD compared to traditional EV certs.

Traditional Extended Validation (EV) certificates from authorities like DigiCert cost $300-$500/year and require managing Cloud HSMs like Azure Key Vault. Azure Trusted Signing is the modern, integrated solution.

1. **Setup Azure Account:** Create an Azure account and set up a **Trusted Signing Account** resource.
2. **Identity Validation:** Complete the identity validation process within Azure (this proves you are a legitimate entity).
3. **Configure GitHub Actions:** Use the `azure/trusted-signing-action` in your workflow, which allows GitHub Actions to securely call Azure to sign the `.exe` and `.msi` files without you ever handling the private key.

### The Alternative Method (Free)
If you do not sign the Windows binaries, users will see a prominent blue **"Windows protected your PC"** popup.
**User workaround:** Users must click **"More info"** and then **"Run anyway"**.

---

## Automating the Release (GitHub Actions)

Once the certificates/cloud signing are configured, Tauri handles the heavy lifting. The `tauri-action` will automatically read the `APPLE_CERTIFICATE` and `APPLE_CERTIFICATE_PASSWORD` environment variables to sign and notarize the macOS build.

For Windows, you add a post-build step to submit the generated `.msi` to Azure Trusted Signing.

> See `.github/workflows/release-gui.yml` for the implementation of the automated signing pipeline.
