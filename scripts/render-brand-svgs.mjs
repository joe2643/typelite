// Derive the desktop badge and macOS menu templates from the checked-in Koelony artwork.
import { readFileSync, writeFileSync } from 'node:fs'

const art = JSON.parse(readFileSync(new URL('../src/brand/koel-art.json', import.meta.url), 'utf8'))
const output = (path, svg) => writeFileSync(new URL(`../${path}`, import.meta.url), svg)
const { cx, cy, r } = art.mark.eye

for (const dev of [false, true]) {
  const dir = dev ? 'src-tauri/icons/dev' : 'src-tauri/icons'
  output(
    `${dir}/app-icon.svg`,
    `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <rect x="96" y="96" width="832" height="832" rx="184" fill="#f4f1ea"/>
  <g transform="translate(211 205) scale(1.18)">
    <path fill="#0c0c0c" d="${art.mark.body}"/>
    <circle cx="${cx}" cy="${cy}" r="${r}" fill="#d0201a"/>
  </g>${dev ? '\n  <rect x="405" y="802" width="214" height="28" rx="14" fill="#7a531b"/>' : ''}
</svg>
`,
  )
  // A transparent eye keeps the menu bar template monochrome under macOS tinting.
  const eye = art.perched.eye
  output(
    dev ? `${dir}/tray.svg` : `${dir}/source/koeltype-tray.svg`,
    `<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 512 512">
  <mask id="eye">
    <rect width="512" height="512" fill="white"/>
    <circle cx="${eye.cx}" cy="${eye.cy}" r="${eye.r * 1.35}" fill="black"/>
  </mask>
  <path fill="#000" mask="url(#eye)" d="${art.perched.body}"/>${dev ? '\n  <circle cx="413" cy="407" r="39" fill="#000"/>' : ''}
</svg>
`,
  )
}

output(
  'site/public/favicon.svg',
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
  <title>KoelType</title>
  <style>
    path { fill: #0c0c0c; }
    @media (prefers-color-scheme: dark) { path { fill: #f4f1ea; } }
  </style>
  <path d="${art.mark.body}"/>
  <circle cx="${cx}" cy="${cy}" r="${r}" fill="#d0201a"/>
</svg>
`,
)
