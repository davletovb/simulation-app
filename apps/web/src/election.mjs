/**
 * Election night, in words and numbers. Pure and dependency-free so `node --test` can check it
 * without a browser. The engine decides the result; this module only describes it.
 *
 * Every per-group array follows `support.group_ids`: urban low, middle, high, then rural.
 */

export const GROUP_LABELS = Object.freeze({
  urban_low: 'Urban, low income',
  urban_middle: 'Urban, middle income',
  urban_high: 'Urban, high income',
  rural_low: 'Rural, low income',
  rural_middle: 'Rural, middle income',
  rural_high: 'Rural, high income',
})

/** A defeat at or above this overall support is "narrow" rather than "heavy" (decision 0004). */
export const NARROW_DEFEAT_PPM = 520000

/**
 * Parts per million as a percentage, truncated to one decimal. Truncating (not rounding) keeps the
 * screen honest next to a bar: 549,999 is "54.9%", never "55.0%", so a number that looks like it
 * meets a bar always does.
 */
export function pct(ppm) {
  return Math.floor(ppm / 1000) / 10
}
export function pctText(ppm) {
  return `${pct(ppm).toFixed(1)}%`
}
/**
 * How far below a bar a value is, in points, rounded **up** to one decimal: being short by a single
 * ppm must read "0.1", never "0.0" (which would look like having met it).
 */
export function shortByText(shortfallPpm) {
  return (Math.ceil(Math.max(0, shortfallPpm) / 1000) / 10).toFixed(1)
}
/** A signed change in percentage points, truncated toward zero: "+1.2" or "−0.9". */
export function pointsText(deltaPpm) {
  const points = Math.trunc(deltaPpm / 1000) / 10
  if (points === 0) return '0.0'
  return `${points > 0 ? '+' : '−'}${Math.abs(points).toFixed(1)}`
}

/**
 * The one bar an election is judged on: overall support, how far the player got and whether it was met. Which
 * groups the support comes from does not matter, so a player can change course and win different ones (0009).
 */
export function supportBar(election) {
  return {
    key: 'support',
    label: 'Overall support',
    value: election.weighted_support_ppm,
    need: election.min_support_ppm,
    met: election.weighted_support_ppm >= election.min_support_ppm,
  }
}

/**
 * Which groups are behind the player, as information only: it never decides the election. A group backs the
 * player strictly above `held_above_ppm`. When every group still sits at exactly the starting 50% nobody has
 * taken a side, which is not the same as everyone being against the player.
 *
 * @returns {{ groups: number, of: number, voters: number, neutral: boolean, text: string }}
 */
export function backing(election) {
  const settled = election.settled_group_ppm
  const groups = settled.filter(ppm => ppm > election.held_above_ppm).length
  const neutral = settled.every(ppm => ppm === election.held_above_ppm)
  const text = neutral ? 'No group has taken a side yet.'
    : groups === 0 ? 'No group backs you yet.'
    : `${groups} of ${settled.length} group${groups === 1 ? '' : 's'} back${groups === 1 ? 's' : ''} you, holding ${pctText(election.blocs_held_ppm)} of the voters.`
  return { groups, of: settled.length, voters: election.blocs_held_ppm, neutral, text }
}

/**
 * The election at the engine's technical ceiling: the longest tenure it can record. Winning it ends the game
 * instead of starting another term. Terms otherwise go on for as long as the player keeps winning, so nothing
 * on screen announces a number of terms ahead of time.
 */
export function isCeiling(election) {
  return election.ceiling === true
}

/**
 * The verdict as the player should read it. While a term runs this is an outlook ("if the
 * election were held now"); once the term has ended it is the result. `final` means "a result, not an
 * outlook"; `ceiling` means this election is at the engine's limit, so winning it ends the game.
 *
 * @returns {{ key: 'reelected'|'tenure'|'narrow'|'heavy'|'on-course'|'behind', label: string, final: boolean, won: boolean, ceiling: boolean }}
 */
export function verdict(election) {
  const ceiling = isCeiling(election)
  if (election.status === 'won') {
    return ceiling
      ? { key: 'tenure', label: 'Longest tenure', final: true, won: true, ceiling }
      : { key: 'reelected', label: 'Re-elected', final: true, won: true, ceiling }
  }
  if (election.status === 'lost') {
    return election.weighted_support_ppm >= NARROW_DEFEAT_PPM
      ? { key: 'narrow', label: 'Narrow defeat', final: true, won: false, ceiling }
      : { key: 'heavy', label: 'Heavy defeat', final: true, won: false, ceiling }
  }
  return supportBar(election).met
    ? { key: 'on-course', label: 'On course to win', final: false, won: true, ceiling }
    : { key: 'behind', label: 'Behind', final: false, won: false, ceiling }
}

/** The headline and one sentence of explanation. */
export function headline(election) {
  const v = verdict(election)
  const support = supportBar(election)
  const supportText = `${pctText(support.value)} overall support`
  if (v.key === 'reelected' || v.key === 'tenure') {
    const cleared = `${supportText} (${pctText(support.need)} needed)`
    return v.key === 'reelected'
      ? { title: 'You’ve been re-elected', lede: `You cleared the bar: ${cleared}.` }
      : {
          title: 'The longest tenure on record',
          lede: `You cleared the bar again: ${cleared}. You have served ${election.term} terms, the most this version can record, so your time in office ends here.`,
        }
  }
  const why = `${supportText} was short of the ${pctText(support.need)} you needed`
  if (v.key === 'narrow') return { title: 'A narrow defeat', lede: `So close: ${why}.` }
  if (v.key === 'heavy') return { title: 'A heavy defeat', lede: `${why}.` }
  return {
    title: v.won ? 'On course to win' : 'Behind',
    lede: v.won ? 'If the election were held now, you would win.' : `If the election were held now you would lose: ${why}.`,
  }
}

/**
 * The six groups as a tidy list for the screen.
 * @param {'current'|'settled'} when `current` is support as of the last quarter; `settled` is once
 *   every queued response has landed (election night).
 */
export function groups(snapshot, when = 'settled') {
  const { support, election } = snapshot
  const values = when === 'current' ? support.group_ppm : election.settled_group_ppm
  return support.group_ids.map((id, i) => ({
    id,
    label: GROUP_LABELS[id] ?? id,
    ppm: values[i],
    weight: support.weight_ppm[i],
    held: values[i] > election.held_above_ppm,
    /** What is already queued to land next quarter. */
    pending: support.pending_delta_ppm[i],
  }))
}

/** Groups that did not back the player, largest first: "who was left behind". */
export function leftBehind(snapshot) {
  return groups(snapshot, 'settled').filter(g => !g.held).sort((a, b) => b.weight - a.weight)
}

/**
 * Legacy: how much of the credits the player was given became delivered work (decision 0001's second
 * line). Version 0 uses only what the treasury already reports; timeliness needs a per-quarter series
 * the snapshot does not carry yet.
 */
export function legacy(snapshot) {
  const t = snapshot.treasury
  const unfinished = t.encumbered_staff + t.encumbered_repairs + t.encumbered_access
  const delivered = t.service_spent
  const total = delivered + unfinished + t.available_credits + t.audit_spent
  const share = total === 0 ? 0 : (delivered / total) * 100
  const [key, label] = share >= 90 ? ['delivered', 'Mandate delivered']
    : share >= 70 ? ['partly', 'Partly delivered']
    : share >= 40 ? ['short', 'Falling short']
    : ['missed', 'Mandate missed']
  return { delivered, unfinished, unspent: t.available_credits, audits: t.audit_spent, total, share, key, label }
}
