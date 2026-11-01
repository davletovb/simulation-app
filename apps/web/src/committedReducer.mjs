/**
 * Pure committed-boundary reducer. Cursors are unsigned decimal strings,
 * not JavaScript Numbers; an older or duplicated SSE frame cannot rewind the
 * world or apply a command twice. A conflicting same-cursor state is fatal.
 */
export function newer(old, next) {
  if (!old) return next
  const a = BigInt(old.snapshot.cursor)
  const b = BigInt(next.snapshot.cursor)
  if (b < a) return old
  if (b === a && old.snapshot.view_version !== next.snapshot.view_version) {
    throw new Error('Conflicting view version at one committed cursor')
  }
  const previous = old.operation
  const incoming = next.operation
  let operation = incoming ?? previous
  if (previous && incoming) {
    if (previous.command_id === incoming.command_id &&
        previous.status !== 'accepted' && incoming.status === 'accepted') {
      operation = previous
    } else if (previous.command_id !== incoming.command_id) {
      // An older completed record from a fallback must not erase an
      // outstanding accepted command, nor replace a newer completion.
      if (previous.status === 'accepted' && incoming.status !== 'accepted') {
        operation = previous
      } else if (previous.committed_cursor != null &&
                 incoming.committed_cursor != null &&
                 BigInt(incoming.committed_cursor) < BigInt(previous.committed_cursor)) {
        operation = previous
      }
    }
  }
  // Preserve the incoming object's identity when no operation merge was
  // necessary: callers can distinguish a genuinely new committed boundary.
  if (b > a && operation === next.operation) return next
  return {snapshot:b === a ? old.snapshot : next.snapshot,operation}
}

/** Decimal cursors must match the entire string, including at its end. */
export function validCommittedCursor(value) {
  // JS $ may match before a trailing newline, so compare the full match.
  return typeof value === 'string' && /^[0-9]+$/.exec(value)?.[0] === value
}

/** The most cells (the finest groups of voters) and the most named groups the engine allows (decision 0015). */
const MAX_CELLS = 64
const MAX_NAMED_GROUPS = 128
const ppm = n => Number.isInteger(n) && n >= 0 && n <= 1000000
/** A list with exactly one entry per cell, each passing `ok`. The number of cells is whatever the campaign's population has. */
const perCell = (cells, list, ok) => Array.isArray(list) && list.length === cells && list.every(ok)
/** How many cells a snapshot says there are: the length of its list of cell ids, within the engine's bounds, else 0. */
const cellCount = support => Array.isArray(support?.group_ids) && support.group_ids.length >= 1 && support.group_ids.length <= MAX_CELLS
  ? support.group_ids.length : 0

/**
 * Every group a headline or a screen can point at (decision 0015): each cell first, in order, then the groups the scenario
 * names. A group is a set of cells, so its share is the sum of theirs and its support lies between the lowest and highest of
 * theirs. The zeroed integrity-failure frame carries none.
 */
function validGroups(groups, support, cells, integrityFailure) {
  if (!Array.isArray(groups) || groups.length > MAX_CELLS + MAX_NAMED_GROUPS) return false
  if (integrityFailure) return groups.length === 0
  if (groups.length < cells) return false
  const ids = new Set()
  return groups.every((g, i) => {
    if (!g || typeof g !== 'object' || typeof g.id !== 'string' || !/^[a-z0-9_]+$/.test(g.id) || ids.has(g.id)) return false
    ids.add(g.id)
    if (typeof g.label !== 'string' || g.label.length === 0 || !ppm(g.weight_ppm) || !ppm(g.support_ppm) ||
      !Number.isInteger(g.pending_delta_ppm) || Math.abs(g.pending_delta_ppm) > 1000000) return false
    if (!Array.isArray(g.cells) || g.cells.length === 0 || new Set(g.cells).size !== g.cells.length ||
      !g.cells.every(c => Number.isInteger(c) && c >= 0 && c < cells)) return false
    // The first entries are the cells themselves, in order.
    if (i < cells && (g.id !== support.group_ids[i] || g.cells.length !== 1 || g.cells[0] !== i)) return false
    const share = g.cells.reduce((sum, c) => sum + support.weight_ppm[c], 0)
    const mood = g.cells.map(c => support.group_ppm[c])
    return g.weight_ppm === share && g.support_ppm >= Math.min(...mood) && g.support_ppm <= Math.max(...mood)
  })
}

/**
 * Public mood of the cells. The voters' shares sum to a million; the only frame allowed to carry
 * zero shares is the zeroed integrity-failure frame, which is never played from.
 */
function validSupport(support, cells, integrityFailure) {
  if (!support || cells === 0) return false
  const shares = Array.isArray(support.weight_ppm) ? support.weight_ppm.reduce((a, b) => a + b, 0) : NaN
  return perCell(cells, support.group_ids, id => typeof id === 'string' && /^[a-z0-9_]+$/.test(id)) &&
    new Set(support.group_ids).size === cells &&
    perCell(cells, support.group_ppm, ppm) &&
    perCell(cells, support.pending_delta_ppm, n => Number.isInteger(n) && Math.abs(n) <= 1000000) &&
    perCell(cells, support.weight_ppm, ppm) &&
    // What one credit of each kind of work does to each cell (decision 0014): three kinds of work, small integers.
    perCell(cells, support.net_per_credit_ppm, row => Array.isArray(row) && row.length === 3 && row.every(n => Number.isInteger(n) && Math.abs(n) <= 40000)) &&
    (shares === 1000000 || (integrityFailure && shares === 0)) &&
    validGroups(support.groups, support, cells, integrityFailure)
}

// Terms go on for as long as the player keeps winning. Only the engine's technical ceiling bounds them, and the
// browser needs it just to reject nonsense.
const MAX_TERMS = 10
const TERM_QUARTERS = 6
const MAX_QUARTERS = MAX_TERMS * TERM_QUARTERS
/** A fresh allocation is granted each term, and windfalls in the news add a few more (at most 15 each, two headlines in the
 * first term and four in every later one), so nothing the game holds can exceed this. */
const MAX_WINDFALL = 15
const MAX_CREDITS = 100 * MAX_TERMS + MAX_WINDFALL * (2 + 4 * (MAX_TERMS - 1))

/**
 * The election. It is a projection while a term runs and a verdict once the term has ended: either the
 * game is over (`closed`) or a win is waiting on election night for the next term to begin.
 */
function validElection(election, held, cells) {
  return !!election && ['projected', 'won', 'lost'].includes(election.status) &&
    (election.status === 'projected') === !held &&
    Number.isInteger(election.term) && election.term >= 1 && election.term <= MAX_TERMS &&
    election.ceiling === (election.term === MAX_TERMS) &&
    perCell(cells, election.settled_group_ppm, ppm) &&
    ppm(election.weighted_support_ppm) && ppm(election.blocs_held_ppm) &&
    ppm(election.min_support_ppm) && ppm(election.held_above_ppm)
}

/**
 * Terms and the elections already held. A game only goes on after a win, so every earlier election was won,
 * and the verdict on screen is the last one on record.
 */
function validTerms(s, held) {
  const e = s.election
  const past = s.past_elections
  if (!(Number.isInteger(s.term) && s.term >= 1 && s.term <= MAX_TERMS && s.term_quarters === TERM_QUARTERS &&
    // Quarter 0 only appears in the zeroed integrity-failure frame.
    (s.quarter === 0 || Math.ceil(s.quarter / TERM_QUARTERS) === s.term) &&
    e.term === s.term && Array.isArray(past) && past.length <= MAX_TERMS)) return false
  const wellFormed = past.every((r, i) => !!r && r.term === i + 1 && r.quarter === (i + 1) * TERM_QUARTERS &&
    typeof r.won === 'boolean' && ppm(r.weighted_support_ppm) && ppm(r.blocs_held_ppm))
  if (!wellFormed) return false
  if (!held) return past.length === s.term - 1 && past.every(r => r.won)
  const last = past.at(-1)
  return past.length === s.term && !!last && last.quarter === s.quarter && last.won === (e.status === 'won') &&
    last.weighted_support_ppm === e.weighted_support_ppm && last.blocs_held_ppm === e.blocs_held_ppm &&
    past.slice(0, -1).every(r => r.won) &&
    // A win ends the game only at the engine's ceiling; any other win waits on election night.
    (s.closed ? (e.status === 'lost' || e.ceiling) : (s.phase === 'ElectionNight' && e.status === 'won'))
}

const FAMILIES = ['staffing', 'repairs', 'access']
const tone = t => t === 'bad' || t === 'good'
const isoDay = s => typeof s === 'string' && /^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/.test(s)
/** A link to a real story is always https; nothing else is ever shown as a link. */
const httpsLink = u => typeof u === 'string' && u.startsWith('https://') && u.length <= 300 && !/\s/.test(u)
/** Where a real headline came from. Real news carries it and invented news must not (a missing field is an older server). */
const realOk = h => h.source === 'real'
  ? !!h.real && typeof h.real === 'object' && typeof h.real.publisher === 'string' && h.real.publisher.length > 0 &&
    isoDay(h.real.published) && httpsLink(h.real.url) && typeof h.real.rule_id === 'string' && h.real.rule_id.length > 0 &&
    typeof h.real.rule_label === 'string' && h.real.rule_label.length > 0
  : h.real === null
/** The real headlines imported so far, including those that only inform. */
const realNewsOk = (list, quarter) => Array.isArray(list) && list.length <= 40 && list.every(n => !!n && typeof n === 'object' &&
  Number.isInteger(n.term) && n.term >= 1 && n.term <= MAX_TERMS && typeof n.title === 'string' && n.title.length > 0 &&
  typeof n.summary === 'string' && n.summary.length > 0 && isoDay(n.published) && httpsLink(n.url) &&
  (n.rule_label === null || (typeof n.rule_label === 'string' && n.rule_label.length > 0)) &&
  (n.quarter === null || (Number.isInteger(n.quarter) && n.quarter >= 2 && n.quarter <= MAX_QUARTERS && n.rule_label !== null)))
const keepOk = d => !!d && typeof d === 'object' && FAMILIES.includes(d.family) && Number.isInteger(d.keep_pct) && d.keep_pct >= 0 && d.keep_pct <= 100 &&
  (d.quarters === 1 || d.quarters === 2)
const effectOk = (cells, e) => perCell(cells, e, n => Number.isInteger(n) && Math.abs(n) <= 40000)
/**
 * What an announced headline is quoted to do to one group. A story takes at most 40,000 ppm (the engine's cap on one
 * headline) and a disruption adds what the delayed work would have earned, at most another 40,000, which is not capped
 * again: so a quote can reach twice the cap. A quote beyond that is nonsense. (Refusing a real quote froze the screen on
 * "Saving…", because the update that would have ended it was rejected.)
 */
const quoteOk = (cells, e) => perCell(cells, e, n => Number.isInteger(n) && Math.abs(n) <= 80000)
const unitsOk = u => Array.isArray(u) && u.length === 3 && u.every(n => Number.isInteger(n) && n >= 0 && n <= MAX_CREDITS)
/** Which kinds of work an inspection counts: at least one, each once. */
const countedOk = f => Array.isArray(f) && f.length >= 1 && f.length <= 3 && f.every(x => FAMILIES.includes(x)) && new Set(f).size === f.length
/** What an inspection says it counts (and, before it lands, how much of that it expects to take). A missing field fails. */
const inspectionOk = (cells, s, expected) => !!s && typeof s === 'object' && countedOk(s.families) && unitsOk(s.owed_units) &&
  (!expected || effectOk(cells, s.effect_ppm))
/** A windfall's credits: a whole number, at least one and at most the largest windfall. */
const grantOk = g => g === null || (Number.isInteger(g) && g >= 1 && g <= MAX_WINDFALL)
const wordsOk = h => typeof h.id === 'string' && h.id.length > 0 && typeof h.title === 'string' && h.title.length > 0 &&
  typeof h.summary === 'string' && ['invented', 'real'].includes(h.source)

/**
 * The news calendar. Upcoming headlines are only ever the ones announced, never a surprise, and are at most the two
 * nearest quarters; a bad one carries a price unless it has been answered. Landed headlines are in order.
 */
function validCalendar(calendar, quarter, cells) {
  if (!calendar || !Array.isArray(calendar.upcoming) || !Array.isArray(calendar.recent)) return false
  if (calendar.upcoming.length > 2 || calendar.recent.length > MAX_QUARTERS) return false
  const upcomingOk = calendar.upcoming.every(h => wordsOk(h) && realOk(h) && tone(h.tone) && quoteOk(cells, h.effect_ppm) &&
    Number.isInteger(h.quarter) && h.quarter >= quarter && h.quarter <= quarter + 1 &&
    Number.isInteger(h.overall_ppm) && ['serious', 'minor', 'welcome'].includes(h.severity) &&
    h.relief_cut_pct >= 0 && h.relief_cut_pct <= 100 &&
    // Only bad news can stop work, and a stoppage says how much work it delays and what that costs, per group.
    (h.disruption === null || (h.tone === 'bad' && keepOk(h.disruption) &&
      Number.isInteger(h.disruption.delayed_units) && h.disruption.delayed_units >= 0 && h.disruption.delayed_units <= MAX_CREDITS &&
      effectOk(cells, h.disruption.effect_ppm))) &&
    // Only bad news sends inspectors, and an inspection says which work it counts and how much of it would still be owed.
    (h.scrutiny === null || (h.tone === 'bad' && h.disruption === null && inspectionOk(cells, h.scrutiny, true))) &&
    // Only good news brings credits.
    grantOk(h.grant_credits) && (h.grant_credits === null || h.tone === 'good') &&
    (h.tone === 'good' ? h.relief_cost === null && h.paid === null
      : // Answerable (a price), answered (a payment), or neither because it would cost this player nothing; never both.
        (h.relief_cost === null || h.paid === null) &&
        (h.relief_cost === null || (Number.isInteger(h.relief_cost) && h.relief_cost >= 1 && h.relief_cost <= MAX_CREDITS)) &&
        (h.paid === null || (Number.isInteger(h.paid) && h.paid >= 1 && h.paid <= MAX_CREDITS))))
  const landedOk = calendar.recent.every((h, i, all) => wordsOk(h) && realOk(h) && tone(h.tone) && effectOk(cells, h.effect_ppm) &&
    Number.isInteger(h.quarter) && h.quarter >= 1 && h.quarter <= MAX_QUARTERS && Number.isInteger(h.overall_ppm) &&
    typeof h.surprise === 'boolean' && typeof h.responded === 'boolean' &&
    (h.disruption === null || (h.tone === 'bad' && keepOk(h.disruption))) &&
    (h.scrutiny === null || (h.tone === 'bad' && h.disruption === null && inspectionOk(cells, h.scrutiny, false))) &&
    grantOk(h.grant_credits) && (h.grant_credits === null || h.tone === 'good') &&
    (h.tone === 'bad' || !h.responded) && (i === 0 || all[i - 1].quarter < h.quarter))
  return upcomingOk && landedOk && realNewsOk(calendar.real_news, quarter)
}

/** Validate the real SSE/snapshot envelope before admitting it to the cache. */
export function validSnapshot(raw, campaignId) {
  if (typeof raw !== 'object' || raw === null) return false
  const obj = raw
  const s = obj.snapshot
  const op = obj.operation
  if (op !== null && (typeof op !== 'object' || !op ||
      op.campaign_id !== campaignId ||
      typeof op.command_id !== 'string' ||
      !/^[a-z0-9_-]{1,64}$/.test(op.command_id) ||
      !['accepted', 'completed', 'failed'].includes(op.status) ||
      (op.committed_cursor !== null && (typeof op.committed_cursor !== 'string' ||
        !validCommittedCursor(op.committed_cursor))) ||
      (op.message !== null && typeof op.message !== 'string'))) return false
  return !!s && s.schema_version === 1 && s.campaign_id === campaignId &&
    validCommittedCursor(s.cursor) &&
    typeof s.view_version === 'string' && s.view_version.length > 0 &&
    Number.isInteger(s.quarter) && s.quarter >= 0 && s.quarter <= MAX_QUARTERS && typeof s.phase === 'string' &&
    typeof s.closed === 'boolean' && typeof s.continuation_pending === 'boolean' &&
    s.mode === 'offline_scripted' &&
    Array.isArray(s.votes) && s.votes.length <= 36 &&
    Array.isArray(s.decisions) && s.decisions.length<=MAX_QUARTERS &&
    Array.isArray(s.news) && s.news.length<=12 &&
    s.news.every(n=>n && typeof n.title==='string' && typeof n.summary==='string' &&
      (n.kind==='real_source_update' || n.kind==='fictional_simulation_news') &&
      (n.url===null || typeof n.url==='string')) &&
    Array.isArray(s.delivery_history) && s.delivery_history.length<=MAX_QUARTERS &&
    s.delivery_history.every((r,i)=>r.quarter===i+1 &&
      Array.isArray(r.reported_cumulative) && r.reported_cumulative.length===3 &&
      Array.isArray(r.due_cumulative) && r.due_cumulative.length===3 &&
      [...r.reported_cumulative,...r.due_cumulative].every(n=>Number.isSafeInteger(n) && n>=0 && n<=MAX_CREDITS) &&
      Number.isInteger(r.weighted_support_ppm) && r.weighted_support_ppm>=0 && r.weighted_support_ppm<=1000000) &&
    !!s.comparison && ['primary_campaign','exact_recorded_actions','fresh_scripted_branch','verification_failed'].includes(s.comparison.mode) &&
    typeof s.comparison.inherited_shock_stream==='boolean' &&
    typeof s.comparison.post_fork_actor_actions_fixed==='boolean' &&
    !!s.evidence && typeof s.evidence.case_pack==='string' &&
    typeof s.evidence.source_cutoff_utc==='string' &&
    typeof s.evidence.scope==='string' &&
    Array.isArray(s.evidence.indicators) &&
    s.evidence.indicators.length===(s.comparison.mode==='verification_failed'?0:8) &&
    (s.comparison.mode!=='verification_failed' ||
      (s.phase==='IntegrityFailure' && s.news.length===0 && obj.operation?.status==='failed')) &&
    s.evidence.indicators.every(item=>typeof item.variable_id==='string' &&
      typeof item.display_value==='string' && typeof item.source_url==='string' &&
      item.source_url.startsWith('https://') && Array.isArray(item.quality_flags)) &&
    validSupport(s.support, cellCount(s.support), s.comparison.mode==='verification_failed') &&
    validCalendar(s.calendar, s.quarter, cellCount(s.support)) &&
    validElection(s.election, s.closed || s.phase==='ElectionNight', cellCount(s.support)) &&
    validTerms(s, s.closed || s.phase==='ElectionNight') &&
    // What is queued to land is exactly settled support minus current support.
    s.support.pending_delta_ppm.every((d,i)=>d===s.election.settled_group_ppm[i]-s.support.group_ppm[i]) &&
    !!s.treasury && Number.isInteger(s.treasury.relief_spent) &&
    Object.values(s.treasury).every(n=>Number.isInteger(n) && n>=0 && n<=MAX_CREDITS)
}
