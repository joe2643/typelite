// Derive the desktop badge and macOS menu templates from the checked-in Munia artwork.
import { readFileSync, writeFileSync } from 'node:fs'

const art = JSON.parse(readFileSync(new URL('../src/brand/munia-art.json', import.meta.url), 'utf8'))
const output = (path, svg) => writeFileSync(new URL(`../${path}`, import.meta.url), svg)
const { cx, cy, r } = art.mark.eye
const monoEye = art.mono.eye

for (const dev of [false, true]) {
  const dir = dev ? 'src-tauri/icons/dev' : 'src-tauri/icons'
  output(
    `${dir}/app-icon.svg`,
    `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <rect x="96" y="96" width="832" height="832" rx="184" fill="#f4f1ea"/>
  <g transform="translate(184 184) scale(1.28)">
    <path fill="#0c0c0c" d="${art.mark.body}"/>
    <circle cx="${cx}" cy="${cy}" r="${r}" fill="#0c0c0c"/>
    <path fill="#d0201a" d="${art.mark.beak}"/>
  </g>${dev ? '\n  <rect x="405" y="802" width="214" height="28" rx="14" fill="#7a531b"/>' : ''}
</svg>
`,
  )
  // Even-odd cheek cutout keeps the menu bar template crisp under macOS tinting.
  output(
    dev ? `${dir}/tray.svg` : `${dir}/source/munia-tray.svg`,
    `<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 512 512">
  <path fill="#000" fill-rule="evenodd" d="${art.mono.body}"/>
  <circle cx="${monoEye.cx}" cy="${monoEye.cy}" r="${monoEye.r}" fill="#000"/>${dev ? '\n  <circle cx="458" cy="74" r="38" fill="#000"/>' : ''}
</svg>
`,
  )
}
