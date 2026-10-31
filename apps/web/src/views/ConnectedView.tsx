import { useCallback, useEffect, useState } from 'react'
import { draftStatus, type Allocation } from '../game'
import { readStored, writeStored } from '../storage'

interface Vote {
  ministry: string
  stage: string
  position: string
}
interface Snapshot {
  schema_version: number
  campaign_id: string
  cursor: string
  view_version: string
  quarter: number
  phase: string
  closed: boolean
  proposal: { staff: number; repairs: number; access: number } | null
  votes: Vote[]
  treasury:{
    available_credits:number
    encumbered_staff:number
    encumbered_repairs:number
    encumbered_access:number
    service_spent:number
    audit_spent:number
  }
  outcome: string | null
  assumption: string
  mode: 'offline_scripted'
}
interface Operation {
  campaign_id: string
  command_id: string
  status: 'accepted' | 'completed' | 'failed'
  committed_cursor: string | null
  message: string | null
}
interface Committed { snapshot: Snapshot; operation: Operation | null }
type Transport = 'connecting' | 'connected' | 'reconnecting' | 'unavailable'
const CAMPAIGN = 'demo-england'
const root = '/api/campaigns/' + CAMPAIGN
const PENDING_KEY = 'myth-p6.pending-demo-england'

function validSnapshot(raw: unknown): raw is Committed {
  if (typeof raw !== 'object' || raw === null) return false
  const obj = raw as Partial<Committed>
  const s = obj.snapshot
  return !!s && s.schema_version === 1 && s.campaign_id === CAMPAIGN &&
    typeof s.cursor === 'string' && /^\d+$/.test(s.cursor) &&
    typeof s.view_version === 'string' && s.view_version.length > 0 &&
    Number.isInteger(s.quarter) && typeof s.phase === 'string' &&
    typeof s.closed === 'boolean' && s.mode === 'offline_scripted' &&
    Array.isArray(s.votes) && s.votes.length <= 36 &&
    !!s.treasury && Object.values(s.treasury).every(n=>Number.isInteger(n) && n>=0 && n<=100)
}
function newer(old: Committed | null, next: Committed): Committed {
  // Backend SSE ID and view version are a single committed snapshot boundary.
  // Compare integers as BigInt, never float: journal cursors are unbounded u64.
  if (old && BigInt(next.snapshot.cursor) < BigInt(old.snapshot.cursor)) return old
  if (old && old.snapshot.cursor === next.snapshot.cursor &&
    old.snapshot.view_version === next.snapshot.view_version) {
    // Same-boundary status refresh can still update command operation.
    return { snapshot: old.snapshot, operation: next.operation ?? old.operation }
  }
  return next
}
function makeCommandId(): string {
  // Locally generated idempotency key. A lost acknowledgment can be retried
  // with the same ID; never generate a fresh ID for the same pending request.
  return 'op-' + crypto.randomUUID()
}

/** Connected P6 panel. Rust alone owns all world changes and committed reports.
 * This preview cannot dispatch OpenRouter calls and cannot interpret rules. */
type PlayerCommand = { kind: 'advance_slice' } | { kind:'propose' | 'amend'; allocation:Allocation } |
  {kind:'decide';ratify:boolean}
interface Pending {id:string;version:string;action:PlayerCommand}
function readPending():Pending|null {
  try {
    const raw=readStored(PENDING_KEY)
    if (!raw) return null
    const value=JSON.parse(raw) as Partial<Pending>
    if (typeof value.id!=='string' || !/^op-[a-f0-9-]{36}$/.test(value.id)
      || typeof value.version!=='string' || typeof value.action?.kind!=='string') return null
    return value as Pending
  }catch{return null}
}
export function ConnectedView({allocation}:{allocation:Allocation}) {
  const [current, setCurrent] = useState<Committed | null>(null)
  const [transport, setTransport] = useState<Transport>('connecting')
  const [pending, setPending] = useState<Pending | null>(readPending)
  const [error, setError] = useState<string | null>(null)
  const accept = useCallback((raw: unknown) => {
    if (!validSnapshot(raw)) throw new Error('Incorrect versioned server view')
    setCurrent(old => newer(old, raw))
    if (raw.operation && raw.operation.status !== 'accepted') {
      setPending(null)
      writeStored(PENDING_KEY,'')
    }
  }, [])

  const load = useCallback(async () => {
    const response = await fetch(root + '/snapshot', { cache: 'no-store' })
    if (!response.ok) throw new Error('Local Hiyal campaign unavailable')
    accept(await response.json() as unknown)
  }, [accept])

  useEffect(() => {
    let alive = true
    void load().catch(() => { if (alive) setTransport('unavailable') })
    const source = new EventSource(root + '/events')
    source.onopen = () => { if (alive) setTransport('connected') }
    source.onerror = () => { if (alive) setTransport('reconnecting') }
    for (const kind of ['snapshot', 'snapshot_fallback', 'committed']) {
      source.addEventListener(kind, (event: MessageEvent) => {
        if (!alive) return
        try { accept(JSON.parse(event.data) as unknown) }
        catch { setError('Invalid streamed view; refresh the verified snapshot') }
      })
    }
    return () => { alive = false; source.close() }
  }, [load, accept])

  const advance = async (requested:PlayerCommand) => {
    if (!current || current.snapshot.closed) return
    const operation:Pending = pending ?? { id: makeCommandId(), version: current.snapshot.view_version,action:requested }
    setPending(operation)
    writeStored(PENDING_KEY,JSON.stringify(operation))
    setError(null)
    try {
      const response = await fetch(root + '/commands', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          command_id: operation.id,
          expected_world_version: operation.version,
          ...operation.action,
        }),
      })
      if (!response.ok) {
        const detail = response.status === 409
          ? 'Command conflict or stale view. Refresh before making a new decision.'
          : 'Local command service rejected the operation.'
        throw new Error(detail)
      }
      const saved = await response.json() as Operation
      if (saved.command_id !== operation.id) throw new Error('Command ID mismatch')
      if (saved.status !== 'accepted') {
        setPending(null)
        writeStored(PENDING_KEY,'')
      }
      // The committed update arrives through SSE; 202 is NOT a game success.
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Unknown network error')
      // Retain original operation ID after a lost acknowledgment.
    }
  }

  const refresh = () => {
    setError(null)
    void load().catch(() => setError('Verified campaign snapshot unavailable'))
  }
  const snapshot = current?.snapshot
  return (
    <section className="connected-p6" aria-labelledby="connected-p6-title">
      <div className="connected-head">
        <div>
          <p className="eyebrow">P6 · Connected pilot</p>
          <h2 id="connected-p6-title">The decision room</h2>
          <p>Local Hiyal journal · scripted ministers · no paid model calls</p>
        </div>
        <span role="status" className="connected-status">{transport}</span>
      </div>
      {error && <p role="alert" className="connected-error">{error}</p>}
      {!snapshot ? (
        <div className="connected-grid"><p>Loading a verified committed boundary…</p>
          <button type="button" onClick={refresh}>Retry</button></div>
      ) : (
        <>
          <div className="connected-grid">
            <div><small>Quarter</small><strong>{snapshot.quarter} / 6</strong></div>
            <div><small>Current phase</small><strong>{snapshot.phase}</strong></div>
            <div><small>Committed cursor</small><strong>{snapshot.cursor}</strong></div>
            <div><small>State version</small><strong>{snapshot.view_version}</strong></div>
          </div>
          <div className="connected-summary">
            <div>
              <h3>Proposal</h3>
              {snapshot.proposal ? <p>
                Staff: <strong>{snapshot.proposal.staff}</strong> · Repairs: <strong>{snapshot.proposal.repairs}</strong> ·
                Access: <strong>{snapshot.proposal.access}</strong>
              </p> : <p>Awaiting the proposal in this quarter.</p>}
            </div>
            <div>
              <h3>Cabinet positions</h3>
              {snapshot.votes.length ? (
                <ul>{snapshot.votes.slice(-6).map((vote, i) => <li key={i}>
                  {vote.ministry} · {vote.stage}: <strong>{vote.position}</strong>
                </li>)}</ul>
              ) : <p>No committed positions yet.</p>}
            </div>
            <div>
              <h3>Outcome</h3>
              <p>{snapshot.outcome ?? 'Awaiting a final decision'}</p>
            </div>
            <div>
              <h3>Budget consequences · credits</h3>
              <p>Available: <strong>{snapshot.treasury.available_credits}</strong></p>
              <p>Committed: <strong>{snapshot.treasury.encumbered_staff+
                snapshot.treasury.encumbered_repairs+snapshot.treasury.encumbered_access}</strong></p>
              <p>Delivered spending: <strong>{snapshot.treasury.service_spent}</strong></p>
              <p>Audit spending: <strong>{snapshot.treasury.audit_spent}</strong></p>
            </div>
          </div>
          <p className="connected-assumptions">{snapshot.assumption}</p>
          <div className="connected-actions">
            {pending ? (
              <button type="button" onClick={() => void advance(pending.action)}
                disabled={!error}>Retry existing command ID</button>
            ) : (
              <>
                {snapshot.phase === 'QuarterOpen' && <button type="button"
                  disabled={draftStatus(allocation)!=='valid'}
                  onClick={() => void advance({kind:'propose',allocation})}>Submit draft proposal</button>}
                {snapshot.phase === 'AmendmentWindow' && <button type="button"
                  disabled={draftStatus(allocation)!=='valid'}
                  onClick={() => void advance({kind:'amend',allocation})}>Submit amendment</button>}
                {snapshot.phase === 'FinalPositionsFrozen' && snapshot.votes.filter(v=>v.stage==='final').length >= 3 && (
                  <>
                    <button type="button" onClick={() => void advance({kind:'decide',ratify:true})}>Ratify proposal</button>
                    <button type="button" onClick={() => void advance({kind:'decide',ratify:false})}>Defer proposal</button>
                  </>
                )}
                {!snapshot.closed && <button type="button" onClick={() => void advance({kind:'advance_slice'})}>
                  Continue scripted phase</button>}
              </>
            )}
            <button type="button" onClick={refresh}>Refresh committed view</button>
          </div>
          {pending && <p role="status">Operation {pending.id}: submitted or awaiting acknowledgment. Keep this ID for retries.</p>}
          {current?.operation && <p className="connected-assumptions">
            Last operation: {current.operation.command_id} · {current.operation.status}
          </p>}
          <p className="connected-assumptions">Only the Rust service can commit actions. This scripted path does not yet accept custom policy cards or run Experiment C′.</p>
        </>
      )}
    </section>
  )
}
