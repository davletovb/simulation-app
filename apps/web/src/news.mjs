/**
 * The news calendar, in words (decision 0010). Pure and dependency-free so `node --test` can check it without a browser.
 * The engine decides what a headline does; this module only describes it, and decides whether the Respond button may be
 * offered (the server stays the authority).
 *
 * Every per-cell array follows `support.group_ids`, whatever the number of cells (decision 0015). `labels` is an optional map from
 * group id to the engine's label (`groupLabels(snapshot.support)`).
 */
import { labelOf, pointsText } from './election.mjs'

/** Which groups a headline moves most: those taking at least half as much as the hardest hit (or the biggest gain). */
export function whoItHits(effectPpm, groupIds, labels) {
  const size = effectPpm.map(Math.abs)
  const biggest = Math.max(...size)
  if (biggest === 0) return 'Nobody much'
  const named = groupIds.filter((_, i) => size[i] * 2 >= biggest)
  return named.length === groupIds.length ? 'Every group' : named.map(id => labelOf(id, labels)).join('; ')
}

/** A trade-off story (decision 0014): it helps some groups and hurts others at once. */
export function isTradeOff(effectPpm) {
  return effectPpm.some(x => x > 0) && effectPpm.some(x => x < 0)
}

/** Which groups a story helps and which it hurts, in words, from the signs of what it does to each. */
export function whoGainsAndLoses(effectPpm, groupIds, labels) {
  const names = pick => groupIds.filter((_, i) => pick(effectPpm[i])).map(id => labelOf(id, labels)).join('; ')
  return { helps: names(x => x > 0), hurts: names(x => x < 0) }
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

const FAMILY_WORD = { staffing: 'Staffing', repairs: 'Repairs', access: 'Access' }

/**
 * What a disruption does, in words: which work, how much of its usual capacity it keeps, for how long. `delayedUnits` is
 * only known for news that has yet to land, and says how much of the player's own promised work would slip past the election.
 */
export function disruptionText(disruption, { delayedUnits } = {}) {
  const work = FAMILY_WORD[disruption.family] ?? disruption.family
  const how = disruption.keep_pct === 0 ? 'stops' : `slows to ${disruption.keep_pct}%`
  const long = disruption.quarters === 1 ? 'for a quarter' : `for ${disruption.quarters} quarters`
  const base = `${work} work ${how} ${long}`
  if (delayedUnits === undefined) return `${base}.`
  if (delayedUnits === 0) return `${base}. None of the work you have promised would be delayed past the election.`
  return `${base}. About ${delayedUnits} unit${delayedUnits === 1 ? '' : 's'} of the work you have promised would arrive after the election instead of before it.`
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
/** `2026-11-02` as "2 Nov 2026". Read from the text, never through a time zone. */
export function dayText(iso) {
  const [year, month, day] = iso.split('-').map(Number)
  return `${day} ${MONTHS[month - 1]} ${year}`
}

/** Where a real headline came from and why it does what it does (decision 0013), in one sentence. */
export function realSourceText(real) {
  return `Real news from ${real.publisher}, ${dayText(real.published)}. It moves support because it matched the rule “${real.rule_label}”.`
}

/** One imported real headline, for the list of real news: whether it changes support, and when. */
export function realNoteText(note) {
  if (note.rule_label === null) return 'Shown for information. It matched none of the rules, so it changes nothing.'
  if (note.quarter === null) return `Matched the rule “${note.rule_label}”, but no quarter of its kind was free this term, so it changes nothing.`
  return `Matched the rule “${note.rule_label}”. It lands at the end of quarter ${note.quarter}.`
}

/** "A", "A and B", "A, B and C". */
function listWords(words) {
  if (words.length <= 1) return words.join('')
  return `${words.slice(0, -1).join(', ')} and ${words.at(-1)}`
}
const WORKS = ['Staffing', 'Repairs', 'Access']
const units = n => `${n} unit${n === 1 ? '' : 's'}`

/**
 * What an inspection counts (decision 0012), in words. `owed_units` follows staffing, repairs, access. For news still to
 * come the count is what would still be owed when the inspectors arrive if nothing changes; once landed it is what they found.
 */
export function inspectionText(scrutiny, { landed = false } = {}) {
  const counted = scrutiny.families.map(f => FAMILY_WORD[f] ?? f)
  const found = scrutiny.owed_units.map((n, i) => (n > 0 ? `${units(n)} of ${WORKS[i]}` : null)).filter(Boolean)
  if (landed) {
    return found.length === 0
      ? `Inspectors checked ${listWords(counted)} and found no promised work unfinished.`
      : `Inspectors counted promised work still unfinished: ${listWords(found)}.`
  }
  const base = `Inspectors will count promised ${listWords(counted)} work that is still unfinished`
  return found.length === 0
    ? `${base}. None of the work you have promised would be unfinished by then.`
    : `${base}: ${listWords(found)} would still be owed by then. A smaller plan has less to count.`
}

/** A windfall, in words: the credits it brings. */
export function windfallText(credits, { landed = false } = {}) {
  const noun = `credit${credits === 1 ? '' : 's'}`
  return landed ? `${credits} ${noun} arrived to spend.` : `Brings ${credits} ${noun} to spend, on top of anything you have left.`
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
