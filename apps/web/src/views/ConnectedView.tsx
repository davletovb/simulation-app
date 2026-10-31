import {newer} from '../committedReducer.mjs'
import { useCallback, useEffect, useState } from 'react'
import {useQuery,useQueryClient} from '@tanstack/react-query'
import { draftStatus, type Allocation } from '../game'
import { readStored, writeStored } from '../storage'
import {ActionButton, AssumptionsDialog} from '../components/ConnectedControls'

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
  decisions: {quarter:number; outcome:string; allocation:{staff:number;repairs:number;access:number}|null;
    final_support:number;final_votes:number}[]
  evidence:{case_pack:string;source_cutoff_utc:string;evidence_pack_sha256:string;
    scenario_sha256:string;attribution:string;scope:string}
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
export interface Committed { snapshot: Snapshot; operation: Operation | null }
type Transport = 'connecting' | 'connected' | 'reconnecting' | 'unavailable'
const requestedCampaign = new URLSearchParams(window.location.search).get('campaign')
export const CAMPAIGN = requestedCampaign && /^[a-z0-9_-]{1,64}$/.test(requestedCampaign)
  ? requestedCampaign : 'demo-england'
export const root = '/api/campaigns/' + CAMPAIGN
const PENDING_KEY = 'myth-p6.pending-' + CAMPAIGN
export const CACHE_KEY = ['committed-campaign',CAMPAIGN] as const

function validSnapshot(raw: unknown): raw is Committed {
  if (typeof raw !== 'object' || raw === null) return false
  const obj = raw as Partial<Committed>
  const s = obj.snapshot
  const op = obj.operation
  if (op !== null && (typeof op !== 'object' || !op ||
      op.campaign_id !== CAMPAIGN ||
      typeof op.command_id !== 'string' ||
      !/^[a-z0-9_-]{1,64}$/.test(op.command_id) ||
      !['accepted', 'completed', 'failed'].includes(op.status) ||
      (op.committed_cursor !== null && (typeof op.committed_cursor !== 'string' ||
        !/^\\d+$/.test(op.committed_cursor))) ||
      (op.message !== null && typeof op.message !== 'string'))) return false
  return !!s && s.schema_version === 1 && s.campaign_id === CAMPAIGN &&
    typeof s.cursor === 'string' && /^\d+$/.test(s.cursor) &&
    typeof s.view_version === 'string' && s.view_version.length > 0 &&
    Number.isInteger(s.quarter) && typeof s.phase === 'string' &&
    typeof s.closed === 'boolean' && s.mode === 'offline_scripted' &&
    Array.isArray(s.votes) && s.votes.length <= 36 &&
    Array.isArray(s.decisions) && s.decisions.length<=6 &&
    !!s.evidence && typeof s.evidence.case_pack==='string' &&
    typeof s.evidence.source_cutoff_utc==='string' &&
    typeof s.evidence.scope==='string' &&
    !!s.treasury && Object.values(s.treasury).every(n=>Number.isInteger(n) && n>=0 && n<=100)
}
function exactAllocation(a:Allocation,available:number):boolean{
  return (['staff','repairs','access'] as const).every(k=>
    Number.isSafeInteger(a[k]) && a[k]>=0 && a[k]<=100
  ) && a.staff+a.repairs+a.access<=available
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
export async function loadSnapshot():Promise<Committed> {
  const response=await fetch(root+'/snapshot',{cache:'no-store'})
  if (!response.ok) throw new Error('Local Hiyal campaign unavailable')
  const raw=await response.json() as unknown
  if (!validSnapshot(raw)) throw new Error('Invalid server snapshot')
  return raw
}
export async function fetchCommitted(cache: ReturnType<typeof useQueryClient>): Promise<Committed> {
  const received = await loadSnapshot()
  // Read after HTTP completes: SSE may have advanced the cache meanwhile.
  return newer(cache.getQueryData<Committed>(CACHE_KEY) ?? null, received)
}

export function ConnectedView({allocation}:{allocation:Allocation}) {
  const cache=useQueryClient()
  const query=useQuery<Committed>({
    queryKey:CACHE_KEY,
    queryFn:()=>fetchCommitted(cache),
    staleTime:2000,
  })
  const current=query.data??null
  const [transport, setTransport] = useState<Transport>('connecting')
  const [pending, setPending] = useState<Pending | null>(readPending)
  const [error, setError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const [forkId, setForkId] = useState<string|null>(null)
  const accept = useCallback((raw: unknown) => {
    if (!validSnapshot(raw)) throw new Error('Incorrect versioned server view')
    cache.setQueryData<Committed>(CACHE_KEY,old=>newer(old??null,raw))
    if (raw.operation && raw.operation.status !== 'accepted') {
      setPending(old => {
        if (old && old.id === raw.operation?.command_id) {
          writeStored(PENDING_KEY,'')
          return null
        }
        return old
      })
    }
  }, [cache])

  const load = useCallback(async () => {
    const updated=await loadSnapshot()
    accept(updated)
  }, [accept])

  useEffect(() => {
    let alive = true
    const source = new EventSource(root + '/events')
    source.onopen = () => { if (alive) setTransport('connected') }
    source.onerror = () => {
      if (alive) {
        setTransport('reconnecting')
        void cache.invalidateQueries({queryKey:CACHE_KEY})
      }
    }
    for (const kind of ['snapshot', 'snapshot_fallback', 'committed']) {
      source.addEventListener(kind, (event: MessageEvent) => {
        if (!alive) return
        try { accept(JSON.parse(event.data) as unknown) }
        catch { setError('Invalid streamed view; refresh the verified snapshot') }
      })
    }
    return () => { alive = false; source.close() }
  }, [accept,cache])

  const advance = async (requested:PlayerCommand) => {
    if (!current || current.snapshot.closed) return
    const operation:Pending = pending ?? { id: makeCommandId(), version: current.snapshot.view_version,action:requested }
    setPending(operation)
    writeStored(PENDING_KEY,JSON.stringify(operation))
    setError(null)
    setSubmitting(true)
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
        void load().catch(() => setError('Refresh the verified snapshot before retrying.'))
        throw new Error('Command conflict or stale view. Review the refreshed state, then submit a new decision.')
      }
      if (!response.ok) {
        // Network failures and ambiguous server errors might follow a
        // durable acceptance; preserve the exact command ID for recovery.
        throw new Error('Local command service rejected the operation; check its durable status before retrying.')
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
    } finally {
      setSubmitting(false)
    }
  }

  // A lost 202/SSE acknowledgement must be recovered from the server's
  // durable command lookup. Never substitute a new command ID.
  useEffect(() => {
    if (!pending) return
    let stopped = false
    const probe = async () => {
      try {
        const response = await fetch(root+'/commands/'+encodeURIComponent(pending.id), {cache:'no-store'})
        if (stopped) return
        if (response.status === 404) {
          setError('This command was not found after reconnecting. Retry using the same ID; no new action will be created if it was admitted meanwhile.')
          return
        }
        if (!response.ok) return
        const status = await response.json() as Operation
        if (status.command_id!==pending.id) return
        if (status.status !== 'accepted') {
          await load()
          if (stopped) return
          setPending(null)
          writeStored(PENDING_KEY,'')
          if (status.status==='failed') setError(status.message || 'The command failed; inspect the saved journal.')
        }
      }catch { /* keep the same command ID until explicit retry */ }
    }
    void probe()
    const timer=window.setInterval(() => {void probe()}, 1800)
    return () => {stopped=true;window.clearInterval(timer)}
  },[pending,load])

  const createFork = async () => {
    if (!current || current.snapshot.phase !== 'Resolved') return
    setError(null)
    const branchId='fork-'+crypto.randomUUID().replaceAll('-','').slice(0,16)
    try {
      const response=await fetch(root+'/forks',{
        method:'POST',headers:{'Content-Type':'application/json'},
        body:JSON.stringify({
          branch_id:branchId,
          expected_cursor:current.snapshot.cursor,
          expected_world_version:current.snapshot.view_version,
        }),
      })
      if (!response.ok) throw new Error('Could not create a verified decision-boundary fork')
      const record=await response.json() as {branch_id:string;source_cursor:string}
      if (record.branch_id!==branchId || record.source_cursor!==current.snapshot.cursor)
        throw new Error('Fork boundary confirmation mismatch')
      setForkId(record.branch_id)
    }catch(reason){setError(reason instanceof Error?reason.message:'Cannot create fork')}
  }

  const createCampaign = async () => {
    try {
      const response = await fetch(root, {method:'POST'})
      if (!response.ok) throw new Error('Could not start this local campaign')
      const created = await response.json() as unknown
      accept(created)
      setError(null)
    } catch(reason) {
      setError(reason instanceof Error ? reason.message : 'Cannot start campaign')
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
      {query.isError && !snapshot && <p role="alert">The local campaign could not be loaded.</p>}
      {!snapshot ? (
        <div className="connected-grid"><p>Loading a verified committed boundary…</p>
          <ActionButton onClick={() => void createCampaign()}>Start or open this campaign</ActionButton>
          <ActionButton onClick={refresh}>Retry loading</ActionButton></div>
      ) : (
        <>
          <div className="connected-grid">
            <div><small>Local campaign</small><strong>{CAMPAIGN}</strong></div>
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
              {snapshot.decisions.length>0 && <p>Recorded decisions: {snapshot.decisions.length}</p>}
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
          <AssumptionsDialog casePack={snapshot.evidence.case_pack}
            cutoff={snapshot.evidence.source_cutoff_utc}
            scope={snapshot.evidence.scope} attribution={snapshot.evidence.attribution}/>
          <div className="connected-actions">
            {pending ? (
              <ActionButton onClick={() => void advance(pending.action)} disabled={submitting}>
                Retry existing command ID</ActionButton>
            ) : (
              <>
                {snapshot.phase === 'QuarterOpen' && <ActionButton tone="primary"
                  disabled={draftStatus(allocation)!=='valid' || !exactAllocation(allocation,snapshot.treasury.available_credits)}
                  onClick={() => void advance({kind:'propose',allocation})}>Submit draft proposal</ActionButton>}
                {snapshot.phase === 'AmendmentWindow' && <ActionButton tone="primary"
                  disabled={draftStatus(allocation)!=='valid' || !exactAllocation(allocation,snapshot.treasury.available_credits)}
                  onClick={() => void advance({kind:'amend',allocation})}>Submit amendment</ActionButton>}
                {snapshot.phase === 'FinalPositionsFrozen' && snapshot.votes.filter(v=>v.stage==='final').length >= 3 && (
                  <>
                    <ActionButton tone="primary" onClick={() => void advance({kind:'decide',ratify:true})}>Ratify proposal</ActionButton>
                    <ActionButton onClick={() => void advance({kind:'decide',ratify:false})}>Defer proposal</ActionButton>
                  </>
                )}
                {!snapshot.closed && <ActionButton onClick={() => void advance({kind:'advance_slice'})}>
                  Continue scripted phase</ActionButton>}
              </>
            )}
            {snapshot.phase==='Resolved' && !pending && <ActionButton onClick={() => void createFork()}>Fork this verified decision</ActionButton>}
            <ActionButton onClick={refresh}>Refresh committed view</ActionButton>
          </div>
          {forkId && <p role="status">Branch created at the committed decision boundary.
            <a href={'?campaign='+encodeURIComponent(forkId)}>Open independent branch {forkId}</a>.
            Its journal retains the source shock schedule, and new decisions cannot alter the source campaign.
          </p>}
          {pending && <p role="status">Operation {pending.id}: awaiting acknowledgment or retry. The same ID is preserved. Keep this ID for retries.</p>}
          {current?.operation && <p className="connected-assumptions">
            Last operation: {current.operation.command_id} · {current.operation.status}
          </p>}
          <p className="connected-assumptions">Only the Rust service can commit actions. This scripted path does not yet accept custom policy cards or run Experiment C′.</p>
        </>
      )}
    </section>
  )
}
