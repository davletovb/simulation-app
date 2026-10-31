import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { newer } from '../committedReducer.mjs'
import { BudgetFigures } from '../components/BudgetFigures'
import { Icon } from '../components/Icon'
import { JournalStamp } from '../components/JournalStamp'
import { FAMILIES } from '../game'
import { formatDay, formatPpm, formatUtc, humanPhase, humanize, pillTone, shortHash, stageLabel } from '../format'
import { answeredMinistries, positionsFor } from '../positions.mjs'
import { CACHE_KEY, CAMPAIGN, loadSnapshot, useCommittedCampaign, type Committed, type Snapshot } from './ConnectedView'

/** Polls the saved campaign and reduces every response into the shared committed cache. */
export function usePublicView() {
  const cache = useQueryClient()
  const {data} = useCommittedCampaign()
  const [isError, setIsError] = useState(false)
  useEffect(() => {
    let active = true
    // Other tabs are visible while the command room's SSE listener is
    // unmounted. Poll them, but atomically reduce each HTTP result into
    // the shared cache. A stale response cannot roll back an SSE boundary.
    const update = async () => {
      try {
        const received = await loadSnapshot()
        if (!active) return
        cache.setQueryData<Committed>(CACHE_KEY,old=>newer(old??null,received))
        setIsError(false)
      } catch {
        if (active) setIsError(true)
      }
    }
    void update()
    const interval = window.setInterval(() => { void update() }, 5000)
    return () => { active = false; window.clearInterval(interval) }
  }, [cache])
  return {data,isError}
}

function Pending({ title, error }: { title: string; error: boolean }) {
  return <section className="panel" aria-live="polite">
    <div className="panel-head">
      <h2>{title}</h2>
      <span className="tag tag-calm">{error ? 'Unavailable' : 'Loading'}</span>
    </div>
    <p className="forecast-empty">
      <Icon name={error ? 'alert' : 'hourglass'} size={20} />
      <span>{error ? 'The local service is unavailable. No outcome can be verified.'
        : 'Loading a verified saved campaign snapshot…'}</span>
    </p>
  </section>
}

const sentence = (text: string): string => `${text.replace(/\.+$/, '')}.`

/** The vote slot of one board seat: real recorded positions, or an honest reason there are none. */
export function SeatPositions({ ministry, seat, snapshot, unavailable }: {
  ministry: string
  seat: string
  snapshot: Snapshot | undefined
  unavailable: boolean
}) {
  const mine = snapshot ? positionsFor(snapshot.votes, ministry) : []
  if (mine.length === 0) {
    return (
      <div className="vote-slot">
        <span className="vote-token" aria-hidden="true">?</span>
        <span>
          <strong>{snapshot ? 'No position yet' : unavailable ? 'Service unavailable' : 'Loading…'}</strong>
          {snapshot ? 'Recorded positions appear here once this seat answers.'
            : unavailable ? 'Start the local service to see recorded positions.'
            : 'Reading the saved campaign.'}
        </span>
      </div>
    )
  }
  return (
    <div className="vote-slot vote-slot-live">
      <ul className="positions" aria-label={`${seat} recorded positions`}>
        {mine.map(vote => (
          <li key={vote.stage}>
            <span>{stageLabel(vote.stage)}</span>
            <span className={`pill pill-${pillTone(vote.position)}`}>{vote.position}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}

/** Public projections ONLY: never render raw private world/role observations. */
export function CabinetMeetingStatus({ snapshot, unavailable }: { snapshot: Snapshot | undefined; unavailable: boolean }) {
  if (!snapshot) return <Pending title="Meeting status" error={unavailable} />
  const answered = answeredMinistries(snapshot.votes).length
  return (
    <section className="panel" aria-labelledby="connected-board-report">
      <div className="panel-head">
        <h2 id="connected-board-report">Q{snapshot.quarter} meeting</h2>
        <span className="tag tag-calm">Live from the saved campaign</span>
      </div>
      <div className="room-tiles">
        <div className="room-tile">
          <span className="room-label">Phase</span>
          <strong className="room-value">{humanPhase(snapshot.phase)}</strong>
        </div>
        <div className="room-tile">
          <span className="room-label">Seats answered</span>
          <strong className="room-value">{answered}<small> of 3</small></strong>
        </div>
        <div className="room-tile">
          <span className="room-label">Outcome</span>
          {snapshot.outcome
            ? <strong className="room-value"><span className={`pill pill-${pillTone(snapshot.outcome)}`}>{snapshot.outcome}</span></strong>
            : <strong className="room-value room-dim">Not decided</strong>}
        </div>
      </div>
      <p className="panel-foot">
        {answered === 0
          ? 'No positions are recorded this quarter yet. The seats answer once you submit a package.'
          : 'Each seat below shows the positions it has recorded.'}
        {' '}This screen only displays them; the game service enforces ratification.
      </p>
      <JournalStamp campaign={CAMPAIGN} cursor={snapshot.cursor} version={snapshot.view_version} />
    </section>
  )
}

const MODE_LABEL: Record<Snapshot['comparison']['mode'], string> = {
  primary_campaign: 'Primary campaign · live committed game state (not a replay or fork)',
  exact_recorded_actions: 'Explicitly verified recorded-action replay',
  fresh_scripted_branch: 'Independent branch · new scripted actions after the fork',
  verification_failed: 'Verification failure',
}

export function ConnectedCampaignReport() {
  const {data,isError}=usePublicView()
  const snapshot=data?.snapshot
  if (!snapshot) return <Pending title="Campaign record" error={isError}/>
  if (snapshot.comparison.mode==='verification_failed') return <section className="panel panel-bad" role="alert">
    <div className="panel-head">
      <h2>Journal verification failed</h2>
      <span className="tag">Do not continue</span>
    </div>
    <p className="alert alert-danger"><Icon name="alert" size={16} /><span>The command's durable failure status was received.
      The current figures are not verified. Stop the service and inspect the private save; do not continue
      gameplay or trust a zeroed fallback snapshot.</span></p>
  </section>
  const latest = snapshot.delivery_history.at(-1)
  const comparison = snapshot.comparison
  return <section className="panel" aria-labelledby="connected-campaign-report">
    <div className="panel-head">
      <h2 id="connected-campaign-report">Campaign record</h2>
      <span className="tag">Q{snapshot.quarter} · {humanPhase(snapshot.phase)} · {snapshot.closed ? 'Complete' : 'Active'}</span>
    </div>
    <p className="panel-lede">What your decisions have committed so far, and what the public delivery reports say.</p>

    <h3 className="section-label">Budget · credits</h3>
    <BudgetFigures treasury={snapshot.treasury} breakdown />

    <h3 className="section-label report-gap">Decisions on record</h3>
    {snapshot.decisions.length ? <ol className="ledger">
      {snapshot.decisions.map((decision,i)=><li key={i} className="ledger-item">
        <span className="phase-num" aria-hidden="true">{i + 1}</span>
        <div className="ledger-body">
          <div className="ledger-title">
            <strong>Decision {i + 1}</strong>
            <span className={`pill pill-${pillTone(decision.outcome)}`}>{decision.outcome}</span>
            <span className="hint">Q{decision.quarter} meeting</span>
          </div>
          <p className="ledger-votes">{decision.final_support} of {decision.final_votes} final votes in support</p>
          {decision.allocation
            ? <div className="card-stats room-stats" aria-label="Allocation">
              {FAMILIES.map(f => <span key={f.id} className={`stat stat-${f.id}`}>
                <Icon name={f.id} size={14} /><span className="sr-only">{f.label}</span>{decision.allocation?.[f.id]}
              </span>)}
            </div>
            : <p className="room-muted">No allocation recorded.</p>}
        </div>
      </li>)}
    </ol> : <p className="forecast-empty"><Icon name="banner" size={20} />
      <span>No completed decision has been recorded yet.</span></p>}

    <h3 className="section-label report-gap">Public delivery reports</h3>
    {snapshot.delivery_history.length ? (
      <>
        <div className="forecast-scroll">
          <table className="ledger-table">
            <caption className="sr-only">Public cumulative activity against schedule for each saved quarter.
              These are reports, not verified actual delivery.</caption>
            <thead>
              <tr>
                <th scope="col">Quarter</th>
                {FAMILIES.map(f => <th key={f.id} scope="col" className={`family-${f.id}`}>
                  <span className="th-family"><Icon name={f.id} size={16} />{f.label}</span>
                  <small>reported / scheduled</small>
                </th>)}
                <th scope="col">Weighted support<small>authored game model</small></th>
              </tr>
            </thead>
            <tbody>{snapshot.delivery_history.map(item => (
              <tr key={item.quarter}>
                <th scope="row">Q{item.quarter}</th>
                {FAMILIES.map((f, i) => {
                  const reported = item.reported_cumulative[i] ?? 0
                  const due = item.due_cumulative[i] ?? 0
                  const behind = reported < due
                  return <td key={f.id} className={`family-${f.id}${behind ? ' is-behind' : ''}`}>
                    <b>{reported}</b><span className="cell-due"> / {due}</span>
                    {behind && <em className="cell-flag">behind</em>}
                  </td>
                })}
                <td className="cell-support">{formatPpm(item.weighted_support_ppm)}</td>
              </tr>
            ))}</tbody>
          </table>
        </div>
        <p className="hint">Public cumulative activity and schedules, not verified actual delivery.</p>
      </>
    ) : <p className="forecast-empty"><Icon name="hourglass" size={20} />
      <span>No quarter has advanced yet. A proposal and vote do not imply delivery.</span></p>}

    {(latest?.staff_slowdown_next || latest?.access_bonus_next) && <ul className="pressures report-gap">
      {latest.staff_slowdown_next && <li className="pressure pressure-risk">
        <Icon name="alert" size={16} /><span><strong>Next-quarter risk</strong>
          Recorded workforce slowdown; projected throughput may fall.</span></li>}
      {latest.access_bonus_next && <li className="pressure pressure-boost">
        <Icon name="spark" size={16} /><span><strong>Next-quarter effect</strong>
          Recorded community access-capacity bonus.</span></li>}
    </ul>}

    <h3 className="section-label report-gap">Record type</h3>
    <ul className="chips">
      <li className="chip">{MODE_LABEL[comparison.mode]}</li>
      {comparison.source_campaign_id && <li className="chip">
        Forked from {comparison.source_campaign_id} at cursor {comparison.source_cursor}</li>}
      {comparison.mode==='fresh_scripted_branch' && <>
        <li className="chip">Inherited shock stream: {comparison.inherited_shock_stream ? 'yes' : 'unverified'}</li>
        <li className="chip">Post-fork actor actions fixed: {comparison.post_fork_actor_actions_fixed ? 'yes' : 'no'}</li>
      </>}
    </ul>
    {comparison.source_campaign_id && <p className="hint report-hint">
      This is a local counterfactual branch; its post-fork minister actions are not held fixed.</p>}
    {comparison.mode==='fresh_scripted_branch' && <p className="hint report-hint">
      Comparing separate branches is not an exact recorded-action replay.</p>}

    <p className="panel-foot">Treasury balances and public reports are verified projections of the saved
      synthetic state. Implementation effort, audited actual delivery, undisclosed commitments and private
      actor beliefs are not included here. No UK service-delivery or educational impact is empirically predicted.</p>
    <JournalStamp campaign={CAMPAIGN} cursor={snapshot.cursor} version={snapshot.view_version} />
  </section>
}

export function ConnectedEvidenceReport() {
  const {data,isError}=usePublicView()
  const snapshot=data?.snapshot
  if (!snapshot) return <Pending title="Case provenance" error={isError}/>
  const evidence = snapshot.evidence
  return <section className="panel" aria-labelledby="connected-evidence-report">
    <div className="panel-head">
      <h2 id="connected-evidence-report">Case provenance</h2>
      <span className="tag tag-calm">Synthetic case</span>
    </div>
    <p className="panel-lede">{sentence(evidence.attribution)} {sentence(evidence.scope)}</p>

    <div className="room-tiles">
      <div className="room-tile">
        <span className="room-label">Case pack</span>
        <strong className="room-value room-mono">{evidence.case_pack}</strong>
      </div>
      <div className="room-tile">
        <span className="room-label">Source cutoff</span>
        <strong className="room-value"><time dateTime={evidence.source_cutoff_utc}>{formatUtc(evidence.source_cutoff_utc)}</time></strong>
      </div>
      <div className="room-tile">
        <span className="room-label">Fingerprints (SHA-256)</span>
        <strong className="room-value room-mono" title={evidence.evidence_pack_sha256}>{shortHash(evidence.evidence_pack_sha256)}</strong>
        <details className="mechanics">
          <summary>View full fingerprints</summary>
          <div className="mechanics-body">
            <span className="mechanics-tag">Evidence pack</span>
            <code className="hash">{evidence.evidence_pack_sha256}</code>
            <span className="mechanics-tag">Scenario</span>
            <code className="hash">{evidence.scenario_sha256}</code>
          </div>
        </details>
      </div>
    </div>

    <h3 className="section-label report-gap">Archived source indicators</h3>
    <p className="panel-lede">These figures retain their original measurement universes and reference periods.
      They are context for a fictional pilot, not measurements of simulated delivery or actual pupil outcomes.</p>
    <ul className="indicators">
      {evidence.indicators.map(item=><li key={item.variable_id} className="indicator evidence-public_source">
        <p className="indicator-value">{item.display_value}</p>
        <h4>{humanize(item.variable_id)}</h4>
        <p className="indicator-def">{sentence(item.definition)}</p>
        <p className="indicator-scope">{item.scope} · {item.reference_period}</p>
        <ul className="chips" aria-label="Publisher and quality flags">
          <li className="chip">{item.publisher}</li>
          {item.quality_flags.map(flag => <li key={flag} className="chip chip-flag">{humanize(flag)}</li>)}
        </ul>
        <a className="link" href={item.source_url} target="_blank" rel="noopener noreferrer">Archived source reference</a>
      </li>)}
    </ul>

    <h3 className="section-label report-gap">Frozen news and scenario markers</h3>
    <ol className="news">
      {snapshot.news.map((item,i)=>{
        const real = item.kind==='real_source_update'
        return <li key={item.date+'-'+i} className={`news-item ${real ? 'evidence-public_source' : 'evidence-authored_parameter'}`}>
          <time dateTime={item.date}>{formatDay(item.date)}</time>
          <div className="news-body">
            <span className="badge">{real ? 'Archived real-source update' : 'Fictional simulation event'}</span>
            <h4>{item.title}</h4>
            <p>{item.summary}</p>
            {item.url && <a className="link" href={item.url} target="_blank" rel="noopener noreferrer">Original published source</a>}
          </div>
        </li>
      })}
    </ol>

    <p className="panel-foot">Generated outcome reports are simulation outputs. News and country facts must be read
      from the frozen sourced archive; narrative changes are fictional, not factual updates.
      No LLM-authored briefing or news synthesis has been verified in this offline build.</p>
    <p className="panel-foot">A stable case-pack fingerprint proves which authored
      inputs were selected for this simulation. It does not independently verify the
      underlying country facts, news, model behavior or socioeconomic predictions.</p>
    <JournalStamp campaign={CAMPAIGN} cursor={snapshot.cursor} version={snapshot.view_version} />
  </section>
}
