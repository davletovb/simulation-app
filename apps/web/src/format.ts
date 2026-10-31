import { SEATS, type Family } from './game'

/**
 * Display-only helpers for values that arrive from the Rust service. Game
 * logic keeps using the raw wire values; these only change how they read.
 */

const capitalise = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1)

/** `QuarterOpen` -> `Quarter open`, `FinalPositionsFrozen` -> `Final positions frozen`. */
export function humanPhase(phase: string): string {
  return capitalise(phase.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase())
}

/** `allocated_funding_per_pupil_2026_27` -> `Allocated funding per pupil 2026-27`. */
export function humanize(id: string): string {
  const spaced = id
    .replace(/_(\d{4})_(\d{2})\b/g, ' $1-$2')
    .replace(/_/g, ' ')
    .replace(/\b(fsm|fte)\b/gi, match => match.toUpperCase())
    .trim()
  return capitalise(spaced)
}

/** `57857743…517a901` style abbreviation; the full value stays available to the reader. */
export function shortHash(hash: string): string {
  return hash.length > 20 ? `${hash.slice(0, 8)}…${hash.slice(-8)}` : hash
}

/** `2026-10-29T23:59:59Z` -> `29 Oct 2026, 23:59 UTC`; unparseable input is shown as received. */
export function formatUtc(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  const text = date.toLocaleString('en-GB', {
    timeZone: 'UTC', day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit',
  })
  return `${text} UTC`
}

/** `2027-01-01` -> `1 Jan 2027`; unparseable input is shown as received. */
export function formatDay(iso: string): string {
  const date = new Date(`${iso}T00:00:00Z`)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleDateString('en-GB', { timeZone: 'UTC', day: 'numeric', month: 'short', year: 'numeric' })
}

export const formatPpm = (ppm: number): string => `${(ppm / 10000).toFixed(2)}%`

export type SeatTone = 'finance' | Family

/** Maps a wire ministry id (`finance`, `education`, `infrastructure`) onto a board seat and its colour. */
export function seatFor(ministry: string): { name: string; tone: SeatTone } {
  const seat = SEATS.find(s => s.name.toLowerCase() === ministry.toLowerCase())
  return { name: seat?.name ?? humanize(ministry), tone: seat?.owns ?? 'finance' }
}

export type PillTone = 'ok' | 'warn' | 'bad' | 'calm'

/** Colour for a recorded position or decision outcome. Unknown wording stays neutral. */
export function pillTone(word: string): PillTone {
  switch (word) {
    case 'support': case 'ratified': return 'ok'
    case 'conditional': case 'deferred': return 'warn'
    case 'oppose': return 'bad'
    default: return 'calm'
  }
}

export const STAGE_LABEL: Record<string, string> = { first: 'First position', final: 'Final vote' }
export const stageLabel = (stage: string): string => STAGE_LABEL[stage] ?? humanize(stage)
