# Media

The images and GIFs used by the project README. Regenerate the app screenshots from the
repository root:

```sh
npm ci
node tools/scripts/capture-app-screens.mjs
```

The capture uses Chromium (`/Applications/Chromium.app`, or set `CHROMIUM`). Pass
`--out /path/to/directory` to review the PNGs before replacing the tracked files.

| Files | What they show | Source |
|---|---|---|
| `home.png`, `settings-languages.png`, `onboarding.png`, `copy-pill.png`, `recording-pill.png` | The app's windows and the recording and Copy capsules | Real React app components with the mocked Tauri backend in `tools/capture-app/` |
| `*-dark.png` | The same, in dark mode | The same capture harness |
| `hero.gif`, `translate.gif`, `ask.gif` | Dictation, translation and Ask demo clips | The [former website capture harness](https://github.com/koelony/koeltype/tree/8a8841690bbe1df9d4e51eb639a3da5f548c5327/site/capture), which recorded the website demos |

Conventions:

- Stills are PNG, 1600 px wide; the light version has no suffix, the dark one ends in `-dark`.
  Moving shots are GIF, at most 8 MB each.
- Use neutral example text, generic preset names and documentation addresses such as
  `192.0.2.10`: never real names, host names, addresses or keys.
