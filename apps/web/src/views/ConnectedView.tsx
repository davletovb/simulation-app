import {newer} from '../committedReducer.mjs'
import { useCallback, useEffect, useState } from 'react'
import {useQuery,useQueryClient} from '@tanstack/react-query'
import { FAMILIES, committed, draftStatus, type Allocation } from '../game'
import { OFFLINE_PILOT } from '../contracts'
import { humanPhase, humanize, pillTone, seatFor } from '../format'
import { answeredMinistries, positionsFor } from '../positions.mjs'
import { readStored, writeStored } from '../storage'
import {ActionButton, AssumptionsDialog} from '../components/ConnectedControls'
import { BudgetFigures } from '../components/BudgetFigures'
import { Icon } from '../components/Icon'
import { JournalStamp } from '../components/JournalStamp'

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
  continuation_pending:boolean
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
  news:{ date:string; kind:'real_source_update'|'fictional_simulation_news'; title:string;
    summary:string; url:string|null }[]
  delivery_history: {
    quarter:number
    reported_cumulative:[number,number,number]
    due_cumulative:[number,number,number]
    weighted_support_ppm:number
    staff_slowdown_next:boolean
    access_bonus_next:boolean
  }[]
  comparison: {
    mode:'primary_campaign'|'exact_recorded_actions'|'fresh_scripted_branch'|'verification_failed'
    source_campaign_id:string|null
    source_cursor:string|null
    inherited_shock_stream:boolean
    post_fork_actor_actions_fixed:boolean
  }
  evidence:{case_pack:string;source_cutoff_utc:string;evidence_pack_sha256:string;
    scenario_sha256:string;attribution:string;scope:string;
    indicators:{variable_id:string;definition:string;scope:string;publisher:string;
      reference_period:string;display_value:string;unit:string;source_url:string;
      quality_flags:string[]}[]}
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
type PlayerCommand = { kind: 'advance_slice' | 'continue_quarter' | 'finish_continuation' } | { kind:'propose' | 'amend'; allocation:Allocation } |
  {kind:'decide';ratify:boolean} |
  {kind:'order_audit';family:'staffing'|'repairs'|'access';inspected_quarter:number}
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
// The authoritative cache is write-only through accept()->newer().
// A React Query refetch must never overwrite a later SSE boundary: observers
// therefore do not fetch or auto-refetch and subscribe to committed data only.
export function useCommittedCampaign() {
  return useQuery<Committed>({queryKey:CACHE_KEY,enabled:false,staleTime:Infinity})
}

export function ConnectedView({allocation}:{allocation:Allocation}) {
  const cache=useQueryClient()
  const query=useCommittedCampaign()
  const current=query.data??null
  const [transport, setTransport] = useState<Transport>('connecting')
  const [pending, setPending] = useState<Pending | null>(readPending)
  const [error, setError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const [forkId, setForkId] = useState<string|null>(null)
  const [auditFamily,setAuditFamily] = useState<'staffing'|'repairs'|'access'>('repairs')
  const [auditQuarter,setAuditQuarter] = useState(1)
  const [auditAcknowledged,setAuditAcknowledged] = useState(false)
  const [integrityFailure, setIntegrityFailure] = useState(false)
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
    void load().catch(() => setError('Verified campaign snapshot unavailable. Create a campaign explicitly if it does not exist.'))
  }, [load])

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
    source.addEventListener('integrity_failure', (event: MessageEvent) => {
      if (!alive) return
      // Cursor-0 synthetic failure payloads are NOT committed snapshots.
      // Keep the last verified world and block actions until verification.
      try {
        const raw = JSON.parse(event.data) as unknown
        if (typeof raw !== 'object' || raw === null ||
            (raw as {snapshot?: {campaign_id?: string}}).snapshot?.campaign_id !== CAMPAIGN) return
        setIntegrityFailure(true)
        setTransport('unavailable')
        setError('Campaign integrity verification failed. The last verified snapshot is retained; no further commands can be submitted until recovery.')
      } catch {
        setIntegrityFailure(true)
        setTransport('unavailable')
        setError('Invalid integrity alert; stop issuing commands and inspect the local service.')
      }
    })
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
      const response = await fetch(root, {method:'POST',headers:{'Content-Type':'application/json'},body:'{}'})
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
  const available = snapshot?.treasury.available_credits ?? 0
  const draftFits = draftStatus(allocation) === 'valid' && exactAllocation(allocation, available)
  const needsDraft = !!snapshot && (snapshot.phase === 'AmendmentWindow' ||
    (snapshot.phase === 'QuarterOpen' && !snapshot.continuation_pending))
  const finalCount = snapshot ? snapshot.votes.filter(v => v.stage === 'final').length : 0
  const latestReport = snapshot?.delivery_history.at(-1)
  const auditOpen = !!snapshot && !snapshot.closed && snapshot.phase === 'QuarterOpen' &&
    !snapshot.continuation_pending && snapshot.quarter >= 3 &&
    snapshot.delivery_history.length > 0 && !pending
  const auditQuarterShown = snapshot ? Math.min(auditQuarter, snapshot.delivery_history.length) : 1
  const submitBlocked = needsDraft && !pending && !integrityFailure && !draftFits
    ? draftBlockReason(allocation, available) : null
  return (
    <section className="panel panel-live room" aria-labelledby="connected-p6-title">
      <div className="room-head">
        <div>
          <p className="kicker">Live campaign · P7 offline case</p>
          <h2 id="connected-p6-title">The decision room</h2>
          <p className="room-sub">Saved in the local Hiyal journal · scripted ministers · no paid model calls</p>
        </div>
        <div className="room-head-side">
          <span role="status" className={`link-chip link-${transport}`}>
            <span className="link-dot" aria-hidden="true" />{transport}
          </span>
          {snapshot && <AssumptionsDialog casePack={snapshot.evidence.case_pack}
            cutoff={snapshot.evidence.source_cutoff_utc}
            scope={snapshot.evidence.scope} attribution={snapshot.evidence.attribution}/>}
        </div>
      </div>

      {error && <p role="alert" className="alert alert-danger"><Icon name="alert" size={16} /><span>{error}</span></p>}
      {error && !snapshot && <p role="alert" className="alert alert-danger">
        <Icon name="alert" size={16} /><span>The local campaign could not be loaded.</span></p>}

      {!snapshot ? (
        <div className="room-empty">
          <Icon name="hourglass" size={22} />
          <p>Loading a verified committed boundary…</p>
          <div className="room-buttons">
            <ActionButton tone="primary" onClick={() => void createCampaign()}>Start or open this campaign</ActionButton>
            <ActionButton onClick={refresh}>Retry loading</ActionButton>
          </div>
        </div>
      ) : (
        <>
          <div className="room-tiles">
            <div className="room-tile">
              <span className="room-label">Local campaign</span>
              <strong className="room-value room-mono">{CAMPAIGN}</strong>
            </div>
            <div className="room-tile">
              <span className="room-label">Quarter</span>
              <strong className="room-value">{snapshot.quarter}
                <small> of {OFFLINE_PILOT.total_quarters}{snapshot.closed ? ' · complete' : ''}</small></strong>
              <span className="q-pips" aria-hidden="true">
                {Array.from({ length: OFFLINE_PILOT.total_quarters }, (_, i) => (
                  <span key={i} className={`q-pip${snapshot.closed || i < snapshot.quarter - 1 ? ' is-done' : i === snapshot.quarter - 1 ? ' is-now' : ''}`} />
                ))}
              </span>
            </div>
            <div className="room-tile">
              <span className="room-label">Phase</span>
              <strong className="room-value">{humanPhase(snapshot.phase)}</strong>
            </div>
          </div>

          <div className="room-facts">
            <div className="room-fact">
              <h3 className="room-label">Proposal</h3>
              {snapshot.proposal ? (
                <div className="card-stats room-stats">
                  {FAMILIES.map(f => (
                    <span key={f.id} className={`stat stat-${f.id}`}>
                      <Icon name={f.id} size={14} /><span className="sr-only">{f.label}</span>{snapshot.proposal?.[f.id]}
                    </span>
                  ))}
                </div>
              ) : <p className="room-muted">No proposal yet this quarter.</p>}
            </div>
            <div className="room-fact">
              <h3 className="room-label">Cabinet positions</h3>
              {snapshot.votes.length ? (
                <ul className="vote-lines">{answeredMinistries(snapshot.votes).map(ministry => {
                  const seat = seatFor(ministry)
                  return <li key={ministry}>
                    <span className="vote-seat">
                      <span className={`seat-dot seat-${seat.tone}`} aria-hidden="true" />{seat.name}
                    </span>
                    {positionsFor(snapshot.votes, ministry).map(vote => <span key={vote.stage} className="vote-stage">
                      {humanize(vote.stage)}
                      <span className={`pill pill-${pillTone(vote.position)}`}>{vote.position}</span>
                    </span>)}
                  </li>
                })}</ul>
              ) : <p className="room-muted">No positions recorded yet.</p>}
            </div>
            <div className="room-fact">
              <h3 className="room-label">Outcome</h3>
              {snapshot.outcome
                ? <p><span className={`pill pill-${pillTone(snapshot.outcome)}`}>{snapshot.outcome}</span></p>
                : <p className="room-muted">Awaiting a final decision.</p>}
              {snapshot.decisions.length > 0 && <p className="room-muted">
                {snapshot.decisions.length} decision{snapshot.decisions.length === 1 ? '' : 's'} on record</p>}
            </div>
          </div>

          <div className="room-block">
            <h3 className="room-label">Budget · credits</h3>
            <BudgetFigures treasury={snapshot.treasury} />
          </div>

          {latestReport && (
            <div className="room-report">
              <span className="room-label">Public report · Q{latestReport.quarter}</span>
              {FAMILIES.map((f, i) => (
                <span key={f.id} className={`stat stat-${f.id}`}>
                  <Icon name={f.id} size={14} /><span className="sr-only">{f.label}</span>{latestReport.reported_cumulative[i] ?? 0}
                </span>
              ))}
              <span className="hint">Cumulative units as reported, not audited delivery.</span>
            </div>
          )}

          <div className="room-move">
            <h3 className="section-label">Your move</h3>
            <p className="room-hint">{phaseHint(snapshot, finalCount)}</p>
            <div className="room-buttons">
              {integrityFailure ? <p role="alert" className="alert alert-danger">
                <Icon name="alert" size={16} /><span>Command controls suspended pending a verified recovery.</span></p> : pending ? (
                <ActionButton onClick={() => void advance(pending.action)} disabled={submitting}>
                  Retry existing command ID</ActionButton>
              ) : (
                <>
                  {snapshot.phase === 'QuarterOpen' && snapshot.quarter > 1 &&
                    (snapshot.continuation_pending
                      ? <ActionButton tone="primary" onClick={() => void advance({kind:'finish_continuation'})}>
                          Advance the quarter</ActionButton>
                      : <ActionButton onClick={() => void advance({kind:'continue_quarter'})}>
                          Keep programme · skip new meeting</ActionButton>)}
                  {snapshot.phase === 'QuarterOpen' && !snapshot.continuation_pending && <ActionButton tone="primary"
                    disabled={!draftFits} describedBy={submitBlocked ? 'submit-why' : undefined}
                    onClick={() => void advance({kind:'propose',allocation})}>Submit draft proposal</ActionButton>}
                  {snapshot.phase === 'AmendmentWindow' && <ActionButton tone="primary"
                    disabled={!draftFits} describedBy={submitBlocked ? 'submit-why' : undefined}
                    onClick={() => void advance({kind:'amend',allocation})}>Submit amendment</ActionButton>}
                  {snapshot.phase === 'FinalPositionsFrozen' && finalCount >= 3 && (
                    <>
                      <ActionButton tone="primary" onClick={() => void advance({kind:'decide',ratify:true})}>Ratify proposal</ActionButton>
                      <ActionButton onClick={() => void advance({kind:'decide',ratify:false})}>Defer proposal</ActionButton>
                    </>
                  )}
                  {!snapshot.closed && !snapshot.continuation_pending &&
                    <ActionButton onClick={() => void advance({kind:'advance_slice'})}>
                    Continue scripted phase</ActionButton>}
                </>
              )}
              {snapshot.phase === 'Resolved' && !pending && <ActionButton onClick={() => void createFork()}>
                Fork this verified decision</ActionButton>}
            </div>
            {submitBlocked && <p id="submit-why" className="cta-why">
              <Icon name="lock" size={13} /> {submitBlocked}</p>}
          </div>

          {auditOpen && (
            <div className="room-audit">
              <div className="room-audit-head">
                <h3><Icon name="audit" size={18} />Order an audit</h3>
                <span className="hint">From Q3, for completed periods with scheduled work.</span>
              </div>
              <ul className="chips" aria-label="Audit terms">
                <li className="chip">First audit · 0 credits</li>
                <li className="chip">Later audits · 2 credits each</li>
                <li className="chip chip-private"><Icon name="eye-off" size={14} />Findings stay offline</li>
              </ul>
              <div className="audit-fields">
                <label className="field"><span>Audit family</span>
                  <select className="select" value={auditFamily}
                    onChange={e=>setAuditFamily(e.target.value as typeof auditFamily)}>
                    <option value="staffing">Staffing</option><option value="repairs">Repairs</option>
                    <option value="access">Access</option>
                  </select></label>
                <label className="field"><span>Completed quarter</span>
                  <select className="select" value={auditQuarterShown}
                    onChange={e=>setAuditQuarter(Number(e.target.value))}>
                    {snapshot.delivery_history.map(r=><option key={r.quarter} value={r.quarter}>
                      Q{r.quarter}
                    </option>)}
                  </select></label>
              </div>
              <label className="check"><input type="checkbox" checked={auditAcknowledged}
                onChange={e=>setAuditAcknowledged(e.target.checked)}/>
                <span>I understand that the first audit costs 0 credits and later audits cost 2 credits each,
                  and that full findings are available only in the offline private operator report after
                  stopping the API.</span></label>
              <div className="room-buttons">
                <ActionButton disabled={!auditAcknowledged} onClick={() => void advance({
                  kind:'order_audit', family:auditFamily,
                  inspected_quarter:auditQuarterShown,
                })}>Request audit · up to 2 credits</ActionButton>
              </div>
              <details className="mechanics">
                <summary>Where do the findings appear?</summary>
                <div className="mechanics-body">
                  <p>No private findings appear in the browser or SSE. Stop the local API and run{' '}
                    <code>myth-report</code> to inspect the result.</p>
                  <p>This is an operator debugging tool, not a role-scoped chair interface.</p>
                </div>
              </details>
            </div>
          )}

          {forkId && <p role="status" className="alert alert-ok">
            <Icon name="check" size={16} />
            <span>Branch created at the committed decision boundary.{' '}
              <a className="link" href={'?campaign='+encodeURIComponent(forkId)}>Open independent branch {forkId}</a>.
              Its journal retains the source shock schedule, and new decisions cannot alter the source campaign.</span>
          </p>}
          {pending && <p role="status" className="alert alert-warn">
            <Icon name="hourglass" size={16} />
            <span>Command <code className="op-id">{pending.id}</code> is awaiting acknowledgement.
              Retrying reuses this same ID, so nothing is applied twice.</span>
          </p>}

          <details className="mechanics room-guide">
            <summary>How quarters work</summary>
            <div className="mechanics-body">
              <p>In Q2–Q6, choose either a new funded proposal (with a fresh vote) or continue the existing
                programme unchanged. Continuing records your skip-meeting choice first; then explicitly
                advance the quarter. The two journaled steps survive restarts.</p>
              <p>That advances delivery and deadlines without a new meeting; approved but undelivered
                allocations do not reset each quarter.</p>
              <p>Only the Rust service can commit actions. This scripted path does not yet accept custom
                policy cards or run Experiment C′.</p>
            </div>
          </details>

          <div className="room-foot">
            <div className="room-foot-main">
              <JournalStamp campaign={CAMPAIGN} cursor={snapshot.cursor} version={snapshot.view_version} note={false} />
              {current?.operation && <p className="hint">
                Last operation <code className="op-id" title={current.operation.command_id}>
                  {shortOperation(current.operation.command_id)}</code> · {current.operation.status}</p>}
              <p className="hint">{snapshot.assumption}</p>
            </div>
            <ActionButton size="sm" onClick={refresh}><Icon name="reset" size={16} />Refresh view</ActionButton>
          </div>
        </>
      )}
    </section>
  )
}

/** Why the draft cannot be submitted, in words; shown beside the disabled button. */
function draftBlockReason(allocation: Allocation, available: number): string {
  const total = committed(allocation)
  if (available === 0) return 'No credits are available to fund a new package.'
  if (total === 0) return 'Your draft is empty. Build one in the Draft package below.'
  if (total > available) return `Your draft spends ${total} credits but only ${available} are available. Adjust it in the Draft package below.`
  return 'Your draft cannot be submitted yet. Adjust it in the Draft package below.'
}

/** One line of guidance for the current phase, derived from the controls already offered. */
function phaseHint(snapshot: Snapshot, finalCount: number): string {
  if (snapshot.closed) return 'The campaign is complete. Nothing further can be submitted.'
  switch (snapshot.phase) {
    case 'QuarterOpen':
      if (snapshot.continuation_pending) return 'Programme kept for this quarter. Advance the quarter to apply delivery and deadlines.'
      return snapshot.quarter > 1
        ? 'Fund a new package (the board votes again) or keep the current programme unchanged.'
        : 'Submit a package to open this quarter’s board meeting.'
    case 'AmendmentWindow': return 'Change the package once, or keep it as it is.'
    case 'FinalPositionsFrozen':
      return finalCount >= 3
        ? 'Final votes are in. Ratify the proposal or defer it.'
        : 'Waiting for the remaining final votes. Continue the scripted phase.'
    case 'Resolved': return 'This decision is recorded. Fork it to try a different path, or continue the scripted phase.'
    default: return 'Continue the scripted phase to move the meeting along.'
  }
}

/** `op-1b9d…` style abbreviation; the full ID stays in the tooltip. */
const shortOperation = (id: string): string => id.length > 16 ? `${id.slice(0, 11)}…${id.slice(-4)}` : id
