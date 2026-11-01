import test from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

/**
 * Terms go on for as long as the player keeps winning elections (decision 0008). Nothing on screen may promise
 * or count a fixed number of them: before the first election nobody knows whether there will be a second.
 * This reads the sources so that wording cannot creep back in.
 */
const root = new URL('../src/', import.meta.url).pathname
const files = (dir) => readdirSync(dir).flatMap(name => {
  const path = join(dir, name)
  if (name === 'generated') return []
  return statSync(path).isDirectory() ? files(path) : /\.(tsx?|mjs|mts)$/.test(name) ? [path] : []
})

const FORBIDDEN = [
  [/second term/i, 'names a second term as if it were certain'],
  [/two terms/i, 'says there are two terms'],
  [/\bterms? \d+ of \d+/i, 'counts terms out of a total'],
  [/term \$\{[^}]+\} of \$\{/i, 'counts terms out of a total'],
  [/final (victory|election)/i, 'there is no final election; only the engine ceiling ends a won game'],
  [/OFFLINE_PILOT\.terms|snapshot\.terms|election\.terms/, 'reads a fixed number of terms'],
]

test('no source announces a fixed number of terms', () => {
  const found = []
  for (const file of files(root)) {
    const lines = readFileSync(file, 'utf8').split('\n')
    lines.forEach((line, i) => {
      // Comments may explain what the rule is not.
      if (/^\s*(\/\/|\*|\/\*)/.test(line)) return
      for (const [pattern, why] of FORBIDDEN) {
        if (pattern.test(line)) found.push(`${file.replace(root, 'src/')}:${i + 1}: ${why}: ${line.trim()}`)
      }
    })
  }
  assert.deepEqual(found, [])
})
