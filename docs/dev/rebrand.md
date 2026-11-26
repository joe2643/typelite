# KoelType brand and rename checklist

KoelType is the working display name. The product uses Koelony’s warm paper and charcoal with
red for the bird’s eye and primary actions. Slate and amber distinguish Translate and Ask.
The app uses native body type, with Avenir Next for the sidebar wordmark and page titles. The
site uses the same display face and a five-line staff beside the headline. The bird is a new
product silhouette, with two lines cut into its wing. The capsule keeps its dark plate in
both themes, with a red recording dot and clay and amber waveform. Its geometry and controls
are shared with the existing recording workflow.

## When the name changes

- [ ] Update `productName` and window titles in `src-tauri/tauri.conf.json` and the Dev name
      in `tauri.dev-build.conf.json`. The React app, site header, release notes and installer
      workflow read this setting. Check Rust `APP_NAME`, menu and permission text in
      `src-tauri/Info.plist`, HTML titles and the `{{appName}}` locale variable. Keep the
      old migration note in the 1.2.0 release history.
- [ ] Update the npm names in both package manifests and lockfiles, repository and issue links,
      active guides and README, site content and SEO metadata, DMG backdrop text and the promo
      template. Update `src-tauri/src/llm/language_library/mod.rs` if the repository moves again.
      Use the actual release repository in artifacts and verify the preset index and update feed.
- [ ] Regenerate tracked icons with `bash scripts/render-brand-icons.sh` (root `npm ci`,
      `rsvg-convert`, macOS `sips`). Edit `src-tauri/icons/app-icon.svg`,
      `source/koeltype-tray.svg` and `dmg/background.svg`. The script derives the Dev
      variants and the app, website and installer exports. Regenerate media with
      `cd site && npm run capture`. See [capture instructions](../media/README.md).

## Legacy references

| Class | References and follow-up |
|---|---|
| Compatibility | Retain `dev.oratype.mac` and `dev.oratype.mac.dev`, `oratype` binary and Rust crate names, `oratype.db`, existing Application Support and WebKit paths, Keychain services `OraType` / `OraType Dev`, and logging paths. The native login item uses its previous label explicitly. An in-place Tauri update keeps the old `OraType.app` directory, so its launch-agent executable path stays valid. A new download is `KoelType.app`. Verify a signed update in place and fresh install separately. Decide whether to migrate existing bundle filenames and their launch-agent paths together in a later release. |
| Compatibility | The Typelite → OraType migration and Keychain fallback support direct upgrades from older versions. New export filenames use the KoelType slug. Retain the import format markers for dictionaries and preset shares, preference keys, mute recovery flags, CLI and environment variables (including `ORATYPE_*`), Tauri labels, commands, events and Objective-C classes. The site keeps its existing theme storage key. |
| Historical | The README and site record the 1.2.0 and 1.3.0 names. Keep version-specific release notes, old package and app filenames, historical plans, benchmarks, research, eval samples and legacy fixtures. This branch keeps version 1.2.2. Set the release version separately when publishing is authorized. |
| Externally controlled | Keep the existing updater endpoint and public key, and the `Typelite Signing` identity, until a signed upgrade passes on a disposable macOS account. The old GitHub update URL and the new repo URL reach the same current manifest. The GitHub About text, hosted domains, Worker name and signing/Cloudflare secrets belong to their services. The site still builds for `oratype.sennettlau.me` and keeps the `typelite.sennettlau.me` redirect. Confirm the final hostname before touching Worker routes, canonical/OG URLs, sitemap, robots and README. |

## Provenance and sync

Palette values come from Koelony’s public identity as shown on its main site, Koel extension
and demo. The CSS and bird paths were written for KoelType and ship under this repository’s MIT
license. The private design package describes a vendor workflow for the websites and a separate
public export workflow for Koel. KoelType uses checked-in styling and assets and loads installed
fonts. Its build uses public npm and Cargo dependencies and the pinned public llama.cpp source for
the optional sidecar. To sync a future palette change, compare the public identity and update the
desktop and site CSS token blocks, the capsule palette in both, the icon sources and capture
backdrops. Rebuild and check both themes.

## Native release review

The fixture capture exercises the real React screens and recording/Copy capsule components
with synthetic state. `hero.gif`, `translate.gif`, `ask.gif` and `site/public/og.png` come from the
website’s scripted demos. Capture a native version of those demos on an isolated Apple Silicon
macOS account if required. Use non-sensitive generated speech and a disposable text field.
Before signoff, verify microphone and Accessibility permission continuity, login at startup,
Keychain access, built-in AI/speech, native tray and capsule placement, and the signed update
from the last OraType release. Confirm a manually replaced app bundle starts with the saved
settings and login item enabled.
