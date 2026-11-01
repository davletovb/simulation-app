/**
 * The news calendar, in words (decision 0010). Pure and dependency-free so `node --test` can check it without a browser.
 * The engine decides what a headline does; this module only describes it, and decides whether the Respond button may be
 * offered (the server stays the authority).
 *
 * Every per-group array follows `support.group_ids`: urban low, middle, high, then rural.
 */
import { GROUP_LABELS, pointsText } from './election.mjs'

/** Which groups a headline moves most: those taking at least half as much as the hardest hit (or the biggest gain). */
export function whoItHits(effectPpm, groupIds) {
  const size = effectPpm.map(Math.abs)
  const biggest = Math.max(...size)
  if (biggest === 0) return 'Nobody much'
  const named = groupIds.filter((_, i) => size[i] * 2 >= biggest)
  return named.length === groupIds.length ? 'Every group' : named.map(id => GROUP_LABELS[id] ?? id).join('; ')
}

/** "about −1.3 points overall" for news still to land, "−1.3 points overall" once it has. */
export function overallText(overallPpm, { approximate = false } = {}) {
  const points = pointsText(overallPpm)
  if (points === '0.0') return approximate ? 'about no change overall' : 'no change overall'
  return `${approximate ? 'about ' : ''}${points} points overall`
}

/** When it lands, relative to the quarter being played. */
export function landsText(headlineQuarter, currentQuarter) {
  return headlineQuarter <= currentQuarter ? 'at the end of this quarter' : 'at the end of next quarter'
}

/**
 * Whether the player may answer a headline now, and if not, why not (in words for the screen). The engine accepts a
 * response only at the start of a quarter, before a plan is made, for an announced bad headline not yet answered.
 *
 * @param {{ relief_cost: number | null, paid: number | null, tone: string }} headline
 * @param {{ available: number, step: string, busy: boolean }} state  `step` is the turn step (see turn.mjs)
 * @returns {{ ok: boolean, why: string | null }}
 */
export function canRespond(headline, { available, step, busy }) {
  if (headline.tone !== 'bad') return { ok: false, why: null }
  if (headline.paid !== null) return { ok: false, why: null }
  if (headline.relief_cost === null) return { ok: false, why: null }
  if (step !== 'plan') return { ok: false, why: 'You can answer a headline at the start of a quarter, before you plan.' }
  if (available < headline.relief_cost) {
    return { ok: false, why: `You have ${available} credits left to spend and an answer costs ${headline.relief_cost}.` }
  }
  return busy ? { ok: false, why: null } : { ok: true, why: null }
}

/** What an answer buys, in one sentence, for the button's explanation. */
export function reliefText(headline) {
  if (headline.relief_cost === null) return ''
  return `Answering costs ${headline.relief_cost} credit${headline.relief_cost === 1 ? '' : 's'} that will not become delivered work, and takes ${headline.relief_cut_pct}% off the loss.`
}

/** The headlines that landed in the quarter just finished or the one being played: what "just in" should show. */
export function justIn(recent, currentQuarter) {
  return recent.filter(h => h.quarter >= currentQuarter - 1)
}
