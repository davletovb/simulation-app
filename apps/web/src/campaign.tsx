import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { newer, validSnapshot } from './committedReducer.mjs'
import type { Allocation } from './game'
import { readStored, writeStored } from './storage'

/**
 * The saved campaign, shared by every screen.
 *
 * Rust alone owns the world. The browser sends typed moves with an idempotency
 * key and only ever displays committed snapshots. This module is the one place
 * that talks to the service; screens read it through `useCampaign()`.
 */

export interface Vote {
  ministry: string
  stage: string
  position: string
}
export interface Snapshot {
  schema_version: number
  campaign_id: string
  cursor: string
  view_version: string
  quarter: number
  phase: string
  closed: boolean
  continuation_pending: boolean
  proposal: { staff: number; repairs: number; access: number } | null
  votes: Vote[]
  treasury: {
    available_credits: number
    encumbered_staff: number
    encumbered_repairs: number
    encumbered_access: number
    service_spent: number
    audit_spent: number
  }
  decisions: {
    quarter: number; outcome: string; allocation: { staff: number; repairs: number; access: number } | null
    final_support: number; final_votes: number
  }[]
  /** How the public feels, group by group. Every array follows `group_ids` (urban low, middle, high, then rural). */
  support: {
    group_ids: string[]
    /** As of the last resolved quarter; 500,000 is 50%. */
    group_ppm: number[]
    /** What is already queued to land: next quarter's change of mood. */
    pending_delta_ppm: number[]
    /** Each group's share of the voters; sums to 1,000,000. */
    weight_ppm: number[]
  }
  /** The term being played, from 1. Each term is six quarters and ends with an election; a win starts another. */
  term: number
  term_quarters: number
  /** Elections already held, oldest first. Empty during the first term. */
  past_elections: { term: number; quarter: number; won: boolean; weighted_support_ppm: number; blocs_held_ppm: number }[]
  /**
   * Projected while a term runs (every queued response lands), won or lost once the term has ended: on
   * election night after a win, or when the game is over.
   */
  election: {
    status: 'projected' | 'won' | 'lost'
    /** The term this election ends. */
    term: number
    /** Only at the engine's limit on tenure: winning this election ends the game instead of starting a term. */
    ceiling: boolean
    settled_group_ppm: number[]
    weighted_support_ppm: number
    blocs_held_ppm: number
    min_support_ppm: number
    min_blocs_ppm: number
    held_above_ppm: number
  }
  news: {
    date: string; kind: 'real_source_update' | 'fictional_simulation_news'; title: string
    summary: string; url: string | null
  }[]
  delivery_history: {
    quarter: number
    reported_cumulative: [number, number, number]
    due_cumulative: [number, number, number]
    weighted_support_ppm: number
    staff_slowdown_next: boolean
    access_bonus_next: boolean
  }[]
  comparison: {
    mode: 'primary_campaign' | 'exact_recorded_actions' | 'fresh_scripted_branch' | 'verification_failed'
    source_campaign_id: string | null
    source_cursor: string | null
    inherited_shock_stream: boolean
    post_fork_actor_actions_fixed: boolean
  }
  evidence: {
    case_pack: string; source_cutoff_utc: string; evidence_pack_sha256: string
    scenario_sha256: string; attribution: string; scope: string
    indicators: {
      variable_id: string; definition: string; scope: string; publisher: string
      reference_period: string; display_value: string; unit: string; source_url: string
      quality_flags: string[]
    }[]
  }
  outcome: string | null
  assumption: string
  mode: 'offline_scripted'
}
export interface Operation {
  campaign_id: string
  command_id: string
  status: 'accepted' | 'completed' | 'failed'
  committed_cursor: string | null
  message: string | null
}
export interface Committed { snapshot: Snapshot; operation: Operation | null }
export type Transport = 'connecting' | 'connected' | 'reconnecting' | 'unavailable'
/** `missing`: the service is up but this campaign has not been started. `down`: it cannot be reached. */
export type LoadState = 'loading' | 'ready' | 'missing' | 'down'

export type AuditFamily = 'staffing' | 'repairs' | 'access'
export type PlayerCommand =
  | { kind: 'advance_slice' | 'continue_quarter' | 'finish_continuation' | 'begin_term' }
  | { kind: 'propose' | 'amend'; allocation: Allocation }
  | { kind: 'decide'; ratify: boolean }
  | { kind: 'order_audit'; family: AuditFamily; inspected_quarter: number }
interface Pending { id: string; version: string; action: PlayerCommand }

const requestedCampaign = new URLSearchParams(window.location.search).get('campaign')
export const CAMPAIGN = requestedCampaign && /^[a-z0-9_-]{1,64}$/.test(requestedCampaign)
  ? requestedCampaign : 'demo-england'
export const root = '/api/campaigns/' + CAMPAIGN
/** A fresh, valid campaign name (lowercase letters, digits and hyphens: the rule the service enforces). */
export const freshCampaignName = () => `${CAMPAIGN.slice(0, 40)}-${Date.now().toString(36)}`
const PENDING_KEY = 'myth-p6.pending-' + CAMPAIGN
export const CACHE_KEY = ['committed-campaign', CAMPAIGN] as const

/** Plain wording for failures. The raw service text is kept for the technical panel. */
const MSG = {
  stale: 'The game moved on before that move arrived, so nothing changed. Look at the updated screen and try again.',
  rejected: 'The game service didn’t accept that move. Nothing has been applied twice — try again in a moment.',
  failed: 'That move couldn’t be completed, so nothing changed. Check your plan and try again.',
  notFound: 'That move wasn’t found after reconnecting. Trying again is safe: it can’t be applied twice.',
  integrity: 'The saved game failed its integrity check. The last good state is shown, and no more moves can be made until it is recovered.',
  integrityAlert: 'The game service reported a problem it could not explain. Stop playing and check the local service.',
  stream: 'A live update was unreadable. Use “Refresh” in Technical details to reload the saved game.',
  oldSave: 'A saved game with this name already exists, but it was made with older rules and can’t be opened any more. Start a fresh one under a new name instead.',
} as const

/** What to tell the player when a particular move fails. The raw service text stays in Technical details. */
function failureFor(action: PlayerCommand | undefined): string {
  if (action?.kind === 'amend') {
    return 'A plan can only be revised once, and this one already has been. Choose “Keep my plan as it is” to lock it in for the final vote.'
  }
  if (action?.kind === 'propose') {
    return 'That plan couldn’t be funded. It may be more than you can plan with, or it may cut work that a minister’s condition protects. Try a smaller plan.'
  }
  if (action?.kind === 'order_audit') {
    return 'The audit couldn’t be ordered. Only the first audit of each term is free; each later one costs 2 credits from “Left to spend”, which must be at least 2.'
  }
  if (action?.kind === 'begin_term') {
    return 'The next term couldn’t begin. It can only start from election night after a win, and only once.'
  }
  if (action?.kind === 'decide' && action.ratify) {
    return 'The ministers’ votes don’t allow approving this plan. Postpone it instead.'
  }
  return MSG.failed
}

class LoadError extends Error {
  constructor(readonly status: number) { super('Local campaign unavailable') }
}

function makeCommandId(): string {
  // Locally generated idempotency key. A lost acknowledgment can be retried
  // with the same ID; never generate a fresh ID for the same pending request.
  return 'op-' + crypto.randomUUID()
}

function readPending(): Pending | null {
  try {
    const raw = readStored(PENDING_KEY)
    if (!raw) return null
    const value = JSON.parse(raw) as Partial<Pending>
    if (typeof value.id !== 'string' || !/^op-[a-f0-9-]{36}$/.test(value.id)
      || typeof value.version !== 'string' || typeof value.action?.kind !== 'string') return null
    return value as Pending
  } catch { return null }
}

export async function loadSnapshot(): Promise<Committed> {
  const response = await fetch(root + '/snapshot', { cache: 'no-store' })
  if (!response.ok) throw new LoadError(response.status)
  const raw = await response.json() as unknown
  if (!validSnapshot(raw, CAMPAIGN)) throw new Error('Invalid server snapshot')
  return raw
}

// The authoritative cache is write-only through accept()->newer().
// A React Query refetch must never overwrite a later SSE boundary: observers
// therefore do not fetch or auto-refetch and subscribe to committed data only.
export function useCommittedCampaign() {
  return useQuery<Committed>({ queryKey: CACHE_KEY, enabled: false, staleTime: Infinity })
}

export interface Session {
  committed: Committed | null
  snapshot: Snapshot | null
  loadState: LoadState
  transport: Transport
  /** Plain-language problem to show the player, if any. */
  error: string | null
  /** Raw service text behind `error`, for the technical panel only. */
  errorDetail: string | null
  dismissError: () => void
  /** A move has been sent and is not yet confirmed as saved. */
  saving: boolean
  /** The saved move may have been lost in transit; retrying reuses its ID so it cannot apply twice. */
  needsRetry: boolean
  integrityFailure: boolean
  forkId: string | null
  /** The campaign name is taken by a game saved with older rules, which can't be opened. */
  staleSave: boolean
  /**
   * Send one move. With `then`, a follow-up move is sent automatically once this one is
   * committed, and dropped if it fails. (Revising a plan is "amend", then "lock".)
   */
  send: (action: PlayerCommand, options?: { then?: PlayerCommand }) => Promise<void>
  retry: () => void
  createFork: () => Promise<void>
  createCampaign: () => Promise<void>
  refresh: () => void
}

function useCampaignSession(): Session {
  const cache = useQueryClient()
  const query = useCommittedCampaign()
  const committed = query.data ?? null
  const [loadState, setLoadState] = useState<LoadState>('loading')
  const [transport, setTransport] = useState<Transport>('connecting')
  const [pending, setPending] = useState<Pending | null>(readPending)
  const [error, setErrorText] = useState<string | null>(null)
  const [errorDetail, setErrorDetail] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)
  // True from the moment a move is sent until the server has answered. A durable-lookup 404 means nothing
  // while the POST is still being admitted (the longer the game, the longer admission takes).
  const sending = useRef(false)
  const [forkId, setForkId] = useState<string | null>(null)
  const [integrityFailure, setIntegrityFailure] = useState(false)
  const [staleSave, setStaleSave] = useState(false)
  const [followUp, setFollowUp] = useState<{ afterVersion: string; action: PlayerCommand } | null>(null)

  const fail = useCallback((message: string, detail: string | null = null) => {
    setErrorText(message)
    setErrorDetail(detail)
  }, [])
  const clearError = useCallback(() => { setErrorText(null); setErrorDetail(null) }, [])

  const accept = useCallback((raw: unknown) => {
    if (!validSnapshot(raw, CAMPAIGN)) throw new Error('Incorrect versioned server view')
    cache.setQueryData<Committed>(CACHE_KEY, old => newer(old ?? null, raw))
    setLoadState('ready')
    if (raw.operation && raw.operation.status !== 'accepted') {
      const operation = raw.operation
      setPending(old => {
        if (old && old.id === operation.command_id) {
          writeStored(PENDING_KEY, '')
          // A failure that reaches us over the live stream before the status
          // probe must still be reported, never swallowed.
          if (operation.status === 'failed') fail(failureFor(old.action), operation.message)
          return null
        }
        return old
      })
    }
  }, [cache, fail])

  const load = useCallback(async () => {
    accept(await loadSnapshot())
  }, [accept])

  useEffect(() => {
    void load().catch((reason: unknown) => {
      setLoadState(reason instanceof LoadError && reason.status === 404 ? 'missing' : 'down')
    })
  }, [load])

  // The live stream only exists for a campaign that exists: opening it earlier gets a 404,
  // which the browser treats as final, leaving a campaign started later without live updates.
  const live = loadState === 'ready'
  useEffect(() => {
    if (!live) return
    let alive = true
    const source = new EventSource(root + '/events')
    source.onopen = () => { if (alive) setTransport('connected') }
    source.onerror = () => {
      if (alive) {
        setTransport('reconnecting')
        void cache.invalidateQueries({ queryKey: CACHE_KEY })
      }
    }
    source.addEventListener('integrity_failure', (event: MessageEvent) => {
      if (!alive) return
      // Cursor-0 synthetic failure payloads are NOT committed snapshots.
      // Keep the last verified world and block actions until verification.
      try {
        const raw = JSON.parse(event.data) as unknown
        if (typeof raw !== 'object' || raw === null ||
            (raw as { snapshot?: { campaign_id?: string } }).snapshot?.campaign_id !== CAMPAIGN) return
        setIntegrityFailure(true)
        setTransport('unavailable')
        fail(MSG.integrity)
      } catch {
        setIntegrityFailure(true)
        setTransport('unavailable')
        fail(MSG.integrityAlert)
      }
    })
    for (const kind of ['snapshot', 'snapshot_fallback', 'committed']) {
      source.addEventListener(kind, (event: MessageEvent) => {
        if (!alive) return
        try { accept(JSON.parse(event.data) as unknown) }
        catch { fail(MSG.stream) }
      })
    }
    return () => { alive = false; source.close() }
  }, [accept, cache, fail, live])

  const send = useCallback(async (requested: PlayerCommand, options?: { then?: PlayerCommand }) => {
    const current = cache.getQueryData<Committed>(CACHE_KEY)
    if (!current || current.snapshot.closed) return
    const operation: Pending = pending ?? { id: makeCommandId(), version: current.snapshot.view_version, action: requested }
    setFollowUp(options?.then ? { afterVersion: current.snapshot.view_version, action: options.then } : null)
    setPending(operation)
    writeStored(PENDING_KEY, JSON.stringify(operation))
    clearError()
    setSubmitting(true)
    sending.current = true
    try {
      const response = await fetch(root + '/commands', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          command_id: operation.id,
          expected_world_version: operation.version,
          kind: operation.action,
        }),
      })
      if (response.status === 409) {
        // The server definitively rejected this admission. It cannot be a
        // lost 202 acknowledgment: no new durable operation was accepted.
        // Retrying the same stale version/ID forever would trap the player.
        setPending(null)
        writeStored(PENDING_KEY, '')
        void load().catch(() => fail('The saved game could not be reloaded. Use “Refresh” in Technical details.'))
        throw Object.assign(new Error(MSG.stale), { friendly: true })
      }
      if (!response.ok) {
        // Network failures and ambiguous server errors might follow a
        // durable acceptance; preserve the exact command ID for recovery.
        throw Object.assign(new Error(MSG.rejected), { friendly: true })
      }
      const saved = await response.json() as Operation
      if (saved.command_id !== operation.id) throw new Error('Command ID mismatch')
      if (saved.status !== 'accepted') {
        setPending(null)
        writeStored(PENDING_KEY, '')
        if (saved.status === 'failed') fail(failureFor(operation.action), saved.message)
      }
      // The committed update arrives through SSE; 202 is NOT a game success.
    } catch (reason) {
      const known = reason instanceof Error && (reason as { friendly?: boolean }).friendly
      fail(known ? (reason as Error).message : MSG.rejected, reason instanceof Error && !known ? reason.message : null)
      // Retain original operation ID after a lost acknowledgment.
    } finally {
      sending.current = false
      setSubmitting(false)
    }
  }, [cache, clearError, fail, load, pending])

  // A lost 202/SSE acknowledgement must be recovered from the server's
  // durable command lookup. Never substitute a new command ID.
  useEffect(() => {
    if (!pending) return
    let stopped = false
    const probe = async () => {
      try {
        const response = await fetch(root + '/commands/' + encodeURIComponent(pending.id), { cache: 'no-store' })
        if (stopped) return
        // Not found while the move is still being sent just means the server has not admitted it yet.
        if (response.status === 404) { if (!sending.current) fail(MSG.notFound); return }
        if (!response.ok) return
        const status = await response.json() as Operation
        if (status.command_id !== pending.id) return
        if (status.status !== 'accepted') {
          await load()
          if (stopped) return
          setPending(null)
          writeStored(PENDING_KEY, '')
          if (status.status === 'failed') fail(failureFor(pending.action), status.message)
        }
      } catch { /* keep the same command ID until explicit retry */ }
    }
    void probe()
    const timer = window.setInterval(() => { void probe() }, 1800)
    return () => { stopped = true; window.clearInterval(timer) }
  }, [pending, load, fail])

  // Send the queued follow-up only once the first move is fully committed (the world has
  // moved on and nothing is pending). A failure drops it, so it can never reorder or repeat.
  const version = committed?.snapshot.view_version
  useEffect(() => {
    if (!followUp || pending || submitting) return
    if (error) { setFollowUp(null); return }
    if (version !== undefined && version !== followUp.afterVersion) {
      const next = followUp.action
      setFollowUp(null)
      void send(next)
    }
  }, [followUp, pending, submitting, error, version, send])

  const createFork = useCallback(async () => {
    const current = cache.getQueryData<Committed>(CACHE_KEY)
    if (!current || current.snapshot.phase !== 'Resolved') return
    clearError()
    const branchId = 'fork-' + crypto.randomUUID().replaceAll('-', '').slice(0, 16)
    try {
      const response = await fetch(root + '/forks', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          branch_id: branchId,
          expected_cursor: current.snapshot.cursor,
          expected_world_version: current.snapshot.view_version,
        }),
      })
      if (!response.ok) throw new Error('Could not create a verified decision-boundary fork')
      const record = await response.json() as { branch_id: string; source_cursor: string }
      if (record.branch_id !== branchId || record.source_cursor !== current.snapshot.cursor)
        throw new Error('Fork boundary confirmation mismatch')
      setForkId(record.branch_id)
    } catch (reason) {
      fail('The what-if branch couldn’t be created. Your game is unchanged.', reason instanceof Error ? reason.message : null)
    }
  }, [cache, clearError, fail])

  const createCampaign = useCallback(async () => {
    try {
      const response = await fetch(root, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' })
      if (response.status === 409) {
        // The name exists on disk but cannot be opened: it was saved under different rules.
        setStaleSave(true)
        fail(MSG.oldSave, 'campaign could not be created (409)')
        return
      }
      if (!response.ok) throw new Error('Could not start this local campaign')
      accept(await response.json() as unknown)
      clearError()
    } catch (reason) {
      fail('A new campaign couldn’t be started. Is the game service running?', reason instanceof Error ? reason.message : null)
    }
  }, [accept, clearError, fail])

  const refresh = useCallback(() => {
    clearError()
    void load().catch(() => fail('The saved game could not be reloaded. Is the game service running?'))
  }, [clearError, fail, load])

  const retry = useCallback(() => { if (pending) void send(pending.action) }, [pending, send])

  return useMemo<Session>(() => ({
    committed,
    snapshot: committed?.snapshot ?? null,
    loadState, transport, error, errorDetail,
    dismissError: clearError,
    saving: submitting || pending !== null || followUp !== null,
    needsRetry: pending !== null && error !== null && !submitting,
    integrityFailure, forkId, staleSave,
    send, retry, createFork, createCampaign, refresh,
  }), [committed, loadState, transport, error, errorDetail, clearError, submitting, pending, followUp,
    integrityFailure, forkId, staleSave, send, retry, createFork, createCampaign, refresh])
}

const SessionContext = createContext<Session | null>(null)

export function CampaignProvider({ children }: { children: ReactNode }) {
  const session = useCampaignSession()
  return <SessionContext.Provider value={session}>{children}</SessionContext.Provider>
}

export function useCampaign(): Session {
  const session = useContext(SessionContext)
  if (!session) throw new Error('useCampaign must be used inside <CampaignProvider>')
  return session
}
