import { describe, expect, it } from 'vitest'
import css from '../globals.css?raw'

function ruleBody(selectorPattern: RegExp): string {
  const match = css.match(selectorPattern)
  expect(match, `missing CSS rule matching ${selectorPattern}`).not.toBeNull()
  return match![1]
}

describe('dark mode select contrast & Windows backdrop CSS tokens', () => {
  it('propagates light and dark color-scheme onto html and select controls', () => {
    expect(css).toMatch(/html\s*\{\s*color-scheme:\s*light;\s*\}/)
    expect(css).toMatch(/html\.dark\s*\{\s*color-scheme:\s*dark;\s*\}/)
    expect(css).toMatch(/\.dark select\s*\{\s*color-scheme:\s*dark;\s*\}/)
  })

  it('uses an opaque dark surface and light ink for .dark .popup and .dark select.field', () => {
    const darkPopupBody = ruleBody(/\.dark \.popup,\s*\.dark select\.field\s*\{([^}]*)\}/)
    expect(darkPopupBody).toContain('color-scheme: dark')
    expect(darkPopupBody).toMatch(/background-color:\s*#242e42/)
    expect(darkPopupBody).toMatch(/color:\s*#f1f6ff/)
    expect(darkPopupBody).not.toContain('--color-bg-tertiary')
  })

  it('explicitly styles <option> and <optgroup> backgrounds and text in light and dark modes', () => {
    const lightOptions = ruleBody(/select option,\s*select optgroup\s*\{([^}]*)\}/)
    expect(lightOptions).toMatch(/background-color:\s*#ffffff/)
    expect(lightOptions).toMatch(/color:\s*#1d1d1f/)

    const darkOptions = ruleBody(/\.dark select option,\s*\.dark select optgroup\s*\{([^}]*)\}/)
    expect(darkOptions).toMatch(/background-color:\s*#1c2434/)
    expect(darkOptions).toMatch(/color:\s*#f1f6ff/)
  })

  it('provides opaque Windows solid fallback and high-legibility Mica backdrop tokens', () => {
    const solidLight = ruleBody(/html\[data-windows-backdrop='solid'\]\s*\{([^}]*)\}/)
    expect(solidLight).toMatch(/--color-glass:\s*#[0-9a-fA-F]{6}/)
    expect(solidLight).toMatch(/--color-content:\s*#[0-9a-fA-F]{6}/)

    const solidDark = ruleBody(/html\.dark\[data-windows-backdrop='solid'\]\s*\{([^}]*)\}/)
    expect(solidDark).toMatch(/--color-glass:\s*#[0-9a-fA-F]{6}/)
    expect(solidDark).toMatch(/--color-content:\s*#[0-9a-fA-F]{6}/)

    expect(css).toContain("html[data-windows-backdrop='mica']")
    expect(css).toContain("html.dark[data-windows-backdrop='mica']")
  })
})
