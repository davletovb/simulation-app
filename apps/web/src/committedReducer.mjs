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

const GROUPS = 6
const ppm = n => Number.isInteger(n) && n >= 0 && n <= 1000000
const sixOf = (list, ok) => Array.isArray(list) && list.length === GROUPS && list.every(ok)

/**
 * Public mood of the six groups. The voters' shares sum to a million; the only frame allowed to carry
 * zero shares is the zeroed integrity-failure frame, which is never played from.
 */
function validSupport(support, integrityFailure) {
  if (!support) return false
  const shares = Array.isArray(support.weight_ppm) ? support.weight_ppm.reduce((a, b) => a + b, 0) : NaN
  return sixOf(support.group_ids, id => typeof id === 'string' && id.length > 0) &&
    sixOf(support.group_ppm, ppm) &&
    sixOf(support.pending_delta_ppm, n => Number.isInteger(n) && Math.abs(n) <= 1000000) &&
    sixOf(support.weight_ppm, ppm) &&
    (shares === 1000000 || (integrityFailure && shares === 0))
}

/** The election result: a projection while the campaign runs, a verdict once it has closed. */
function validElection(election, closed) {
  return !!election && ['projected', 'won', 'lost'].includes(election.status) &&
    (election.status === 'projected') === !closed &&
    sixOf(election.settled_group_ppm, ppm) &&
    ppm(election.weighted_support_ppm) && ppm(election.blocs_held_ppm) &&
    ppm(election.min_support_ppm) && ppm(election.min_blocs_ppm) && ppm(election.held_above_ppm)
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
    Number.isInteger(s.quarter) && typeof s.phase === 'string' &&
    typeof s.closed === 'boolean' && typeof s.continuation_pending === 'boolean' &&
    s.mode === 'offline_scripted' &&
    Array.isArray(s.votes) && s.votes.length <= 36 &&
    Array.isArray(s.decisions) && s.decisions.length<=6 &&
    Array.isArray(s.news) && s.news.length<=12 &&
    s.news.every(n=>n && typeof n.title==='string' && typeof n.summary==='string' &&
      (n.kind==='real_source_update' || n.kind==='fictional_simulation_news') &&
      (n.url===null || typeof n.url==='string')) &&
    Array.isArray(s.delivery_history) && s.delivery_history.length<=6 &&
    s.delivery_history.every((r,i)=>r.quarter===i+1 &&
      Array.isArray(r.reported_cumulative) && r.reported_cumulative.length===3 &&
      Array.isArray(r.due_cumulative) && r.due_cumulative.length===3 &&
      [...r.reported_cumulative,...r.due_cumulative].every(n=>Number.isSafeInteger(n) && n>=0 && n<=100) &&
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
    validSupport(s.support, s.comparison.mode==='verification_failed') &&
    validElection(s.election, s.closed) &&
    // What is queued to land is exactly settled support minus current support.
    s.support.pending_delta_ppm.every((d,i)=>d===s.election.settled_group_ppm[i]-s.support.group_ppm[i]) &&
    !!s.treasury && Object.values(s.treasury).every(n=>Number.isInteger(n) && n>=0 && n<=100)
}
