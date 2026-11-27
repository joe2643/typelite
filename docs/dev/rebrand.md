# Munia brand and rename checklist

Munia uses Koelony’s Warm Slate palette, official K mark, and red-eyed bird. The shared
palette assigns Harbour Slate to links, selection and focus, and Koel Red to primary buttons.
Chiron GoRound TC is used for Latin display text, with installed fonts for other scripts.
The website’s hero and the app’s Home and welcome screens share a foreground landing
sequence. The hero and Home also show a background flyby. Reduced motion shows the perched
pose immediately. The recording capsule uses the dark palette with Slate and Red indicators.

## When the name changes

- [ ] Update `productName` and window titles in `src-tauri/tauri.conf.json` and the Dev name
      in `tauri.dev-build.conf.json`. The React app, site header, release notes and installer
      workflow read this setting. Check Rust `APP_NAME`, menu and permission text in
      `src-tauri/Info.plist`, HTML titles and the `{{appName}}` locale variable. Keep the
      old migration note in the 1.2.0 release history.
- [ ] Update the npm manifest and lockfile, repository and issue links, active guides, README,
      DMG backdrop text and the promo template. Coordinate site content and SEO changes with
      the website maintainer. Update
      `src-tauri/src/llm/language_library/mod.rs` if the repository moves again.
      Use the actual release repository in artifacts and verify the preset index and update feed.
- [ ] Regenerate tracked icons with `bash scripts/render-brand-icons.sh` (root `npm ci`,
      `rsvg-convert`, macOS `sips`). The script reads the geometry in
      `src/brand/munia-art.json` and the installer layout in
      `src-tauri/icons/dmg/background.svg`, then writes the app, tray and Dev variants. Regenerate
      app screenshots with
      `node tools/scripts/capture-app-screens.mjs`. See [capture instructions](../media/README.md).

## Legacy references

| Class | References and follow-up |
|---|---|
| Compatibility | Retain `dev.oratype.mac` and `dev.oratype.mac.dev`, `oratype` binary and Rust crate names, `oratype.db`, existing Application Support and WebKit paths, Keychain services `OraType` / `OraType Dev`, and logging paths. An in-place Tauri update keeps its installed bundle path. The updated app presents Munia’s name and icon, and fresh downloads use `Munia.app`. Startup registers the Munia login item from the saved preference before retiring legacy items. If registration fails, an existing legacy login item and the enabled preference are kept so startup can retry. Verify a signed in-place update and a fresh install separately. |
| Compatibility | The Typelite → OraType migration and Keychain fallback support direct upgrades from older versions. New preset exports use `.munia-presets.json`. Retain the import format markers for dictionaries and preset shares, preference keys, mute recovery flags, CLI and environment variables (including `ORATYPE_*`), Tauri labels, commands, events and Objective-C classes. The website keeps its existing theme storage key. |
| Historical | Published versions before 1.2.0 used Typelite, and 1.2.x used OraType. KoelType was a working name on the rebrand branch. Keep version-specific release notes, old package and app filenames, historical plans, benchmarks, research, eval samples and legacy fixtures. Set the release version separately when cutting a release. |
| Externally controlled | Keep the existing updater endpoint and public key, and the `Typelite Signing` identity, until a signed upgrade passes on a disposable macOS account. The existing update URL points to the release manifest. The code repository remains `koelony/koeltype`. The GitHub About text, hosted domains, Worker name and signing/Cloudflare secrets belong to their services. The deployed legacy Worker serves `oratype.sennettlau.me` and redirects `typelite.sennettlau.me`. Its [configuration and source](https://github.com/koelony/koeltype/tree/8a8841690bbe1df9d4e51eb639a3da5f548c5327/site) remain in Git history. The separate website and any production routing are managed outside the app release. |

## Provenance and sync

- The colour values and the official K and perched bird geometry come from the public-safe
  Koel brand export (art and palette commits `d81a227`, `56f06fa`). The same mark and
  bird appear on [Koelony’s public website](https://koelony.com/). The geometry and eye are
  retained, with a paper tile for the Dock and a monochrome knockout for the tray.
  This is Koelony brand artwork reused for Munia under the rebrand brief.
- The flying body, wings, and shoulder pivot match the rendered Koelony homepage. The
  landing sequence is a shared CSS adaptation of the public motion. The
  [Koelony homepage](https://koelony.com/) is the reference for the tone-staff flight and
  perch. No separate artwork labelled “bird-fish” was identified in the brand exports.
- The Latin Chiron GoRound TC 700 and 900 WOFF2 subsets are from the deployed Koelony
  homepage. Their [SIL Open Font License](../../src/brand/fonts/OFL.txt) travels with them.
  The [font project](https://github.com/chiron-fonts/chiron-go-round-tc) supplies the
  full family. The app builds locally from these files and public dependencies.

For a palette refresh, compare the next Koel brand export against `src/brand/palette.css`.
Update the light and dark tokens and the fixed dark capsule aliases. For new art, update
`src/brand/koel-art.json`, check the pose alignment in `src/brand/artwork.css`, then regenerate
the app icons and media. Coordinate any corresponding website change separately.

The shared palette has no dedicated **success**, **warning**, **Ask mode** or **capsule
recording and waveform** roles. Munia uses local green and amber for success and warning,
amber with the Ask icon, and dark-theme Red Text and Slate for the capsule. The existing Red
Text role supplies errors, with a local ink mix for readability on tinted backgrounds.
If design adds these roles, ask for light and dark foregrounds and the surfaces they’re
intended for, plus the fixed dark capsule variants.

## Native release review

The fixture capture exercises the real React screens and recording/Copy capsule components
with synthetic state. The README GIFs come from the former website’s scripted demos.
Capture a native version of those demos on an isolated Apple Silicon macOS account if needed. Use non-sensitive generated speech and a disposable text field.
Before signoff, verify microphone and Accessibility permission continuity, login at startup,
Keychain access, built-in AI/speech, native tray and capsule placement, and the signed update
from the last OraType release. Confirm a manually replaced app bundle starts with the saved
settings and login item enabled.
