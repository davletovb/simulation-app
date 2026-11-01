import { BudgetFigures } from '../components/BudgetFigures'
import { Icon } from '../components/Icon'
import { FAMILIES, MINISTERS, quarterLabel } from '../game'
import { dayText, disruptionText, inspectionText, overallText, realNoteText, windfallText } from '../news.mjs'
import { formatDay, formatPpm, formatUtc, humanize, outcomeLabel, pillTone, positionLabel, shortHash, stageLabel } from '../format'
import { answeredMinistries, positionsFor } from '../positions.mjs'
import { STEPS, turnState } from '../turn.mjs'
import { useCampaign, type Snapshot } from '../campaign'

/** The saved campaign as the report screens see it (shared with the Play screen; no separate polling). */
export function usePublicView() {
  const { committed, loadState } = useCampaign()
  return { data: committed ?? undefined, isError: loadState === 'down' || loadState === 'missing' }
}

function Pending({ title, error }: { title: string; error: boolean }) {
  return <section className="panel" aria-live="polite">
    <div className="panel-head">
      <h2>{title}</h2>
      <span className="tag tag-calm">{error ? 'Unavailable' : 'Loading'}</span>
    </div>
    <p className="forecast-empty">
      <Icon name={error ? 'alert' : 'hourglass'} size={20} />
      <span>{error ? 'There is no saved game to show. Start or reconnect to the game on the Play screen.'
        : 'Loading your saved campaign…'}</span>
    </p>
  </section>
}

const sentence = (text: string): string => `${text.replace(/\.+$/, '')}.`

/** One minister's slot: what they have said so far, or an honest reason there is nothing yet. */
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
          <strong>{snapshot ? 'Not asked yet' : unavailable ? 'Unavailable' : 'Loading…'}</strong>
          {snapshot ? 'Their reaction and vote appear here once you put a plan to the cabinet.'
            : unavailable ? 'Start the game on the Play screen.'
            : 'Reading the saved campaign.'}
        </span>
      </div>
    )
  }
  return (
    <div className="vote-slot vote-slot-live">
      <ul className="positions" aria-label={`What the ${seat} minister has said`}>
        {mine.map(vote => (
          <li key={vote.stage}>
            <span>{stageLabel(vote.stage)}</span>
            <span className={`pill pill-${pillTone(vote.position)}`}>{positionLabel(vote.position)}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}

/** Public projections ONLY: never render raw private world/role observations. */
export function CabinetMeetingStatus({ snapshot, unavailable }: { snapshot: Snapshot | undefined; unavailable: boolean }) {
  if (!snapshot) return <Pending title="This quarter’s meeting" error={unavailable} />
  const answered = answeredMinistries(snapshot.votes).length
  const turn = turnState(snapshot)
  const stepName = turn.step === 'done' ? 'Game over'
    : turn.step === 'night' ? 'Election night'
    : turn.keeping ? 'Programme kept'
    : `${turn.index + 1} of ${STEPS.length} · ${STEPS[turn.index]?.label}`
  return (
    <section className="panel" aria-labelledby="connected-board-report">
      <div className="panel-head">
        <h2 id="connected-board-report">Quarter {snapshot.quarter} cabinet meeting</h2>
      </div>
      <div className="room-tiles">
        <div className="room-tile">
          <span className="room-label">Step</span>
          <strong className="room-value">{stepName}</strong>
        </div>
        <div className="room-tile">
          <span className="room-label">Ministers heard</span>
          <strong className="room-value">{answered}<small> of {MINISTERS.length}</small></strong>
        </div>
        <div className="room-tile">
          <span className="room-label">Decision</span>
          {snapshot.outcome
            ? <strong className="room-value"><span className={`pill pill-${pillTone(snapshot.outcome)}`}>{outcomeLabel(snapshot.outcome)}</span></strong>
            : <strong className="room-value room-dim">Not decided yet</strong>}
        </div>
      </div>
      <p className="panel-foot">
        {answered === 0
          ? 'No minister has been asked yet. They respond once you put a plan to the cabinet on the Play screen.'
          : 'This is what each minister has said so far. You make moves on the Play screen.'}
      </p>
    </section>
  )
}

const MODE_LABEL: Record<Snapshot['comparison']['mode'], string> = {
  primary_campaign: 'Your main game',
  exact_recorded_actions: 'A replay of recorded moves',
  fresh_scripted_branch: 'A what-if branch: ministers act afresh after the split',
  verification_failed: 'Verification failed',
}

export function ConnectedCampaignReport() {
  const {data,isError}=usePublicView()
  const snapshot=data?.snapshot
  if (!snapshot) return <Pending title="Campaign record" error={isError}/>
  if (snapshot.comparison.mode==='verification_failed') return <section className="panel panel-bad" role="alert">
    <div className="panel-head">
      <h2>The saved game failed its check</h2>
      <span className="tag">Stop playing</span>
    </div>
    <p className="alert alert-danger"><Icon name="alert" size={16} /><span>The numbers below can’t be trusted. Stop the game service and
      inspect the saved game before playing on.</span></p>
  </section>
  const latest = snapshot.delivery_history.at(-1)
  const comparison = snapshot.comparison
  return <section className="panel" aria-labelledby="connected-campaign-report">
    <div className="panel-head">
      <h2 id="connected-campaign-report">What has happened so far</h2>
      <span className="tag">{snapshot.closed ? 'Campaign complete' : snapshot.phase === 'ElectionNight' ? 'Election night' : `Quarter ${snapshot.quarter}`}</span>
    </div>
    <p className="panel-lede">Your decisions, where the budget stands, and what the ministers say they have delivered.</p>

    <h3 className="section-label">Budget</h3>
    <BudgetFigures treasury={snapshot.treasury} breakdown />

    <h3 className="section-label report-gap">Your decisions</h3>
    {snapshot.decisions.length ? <ol className="ledger">
      {snapshot.decisions.map((decision,i)=><li key={i} className="ledger-item">
        <span className="phase-num" aria-hidden="true">{i + 1}</span>
        <div className="ledger-body">
          <div className="ledger-title">
            <strong>Quarter {decision.quarter}</strong>
            <span className={`pill pill-${pillTone(decision.outcome)}`}>{outcomeLabel(decision.outcome)}</span>
          </div>
          <p className="ledger-votes">{decision.outcome === 'continued'
            ? 'No vote held. The existing programme carried on unchanged.'
            : `${decision.final_support} of ${decision.final_votes} ministers supported it in the final vote`}</p>
          {decision.allocation
            ? <div className="card-stats room-stats" aria-label="Allocation">
              {FAMILIES.map(f => <span key={f.id} className={`stat stat-${f.id}`}>
                <Icon name={f.id} size={14} /><span className="sr-only">{f.label}</span>{decision.allocation?.[f.id]}
              </span>)}
            </div>
            : <p className="room-muted">{decision.outcome === 'continued'
              ? 'There was no funded programme to carry on.' : 'No plan was approved.'}</p>}
        </div>
      </li>)}
    </ol> : <p className="forecast-empty"><Icon name="banner" size={20} />
      <span>No decision has been made yet.</span></p>}

    <h3 className="section-label report-gap">The news so far</h3>
    {snapshot.calendar.recent.length ? <ul className="ledger news-log">
      {[...snapshot.calendar.recent].reverse().map(h => <li key={h.quarter} className={`ledger-item headline is-${h.tone}`}>
        <span className="tag">{quarterLabel(h.quarter).label}</span>
        <div className="ledger-body">
          <div className="ledger-title"><strong>{h.title}</strong></div>
          <p className="ledger-votes">{h.summary}</p>
          {h.disruption && <p className="ledger-votes">{disruptionText(h.disruption)}</p>}
          {h.scrutiny && <p className="ledger-votes">{inspectionText(h.scrutiny, { landed: true })}</p>}
          <p className="ledger-votes">
            {h.surprise ? 'Nobody saw it coming. ' : 'It was announced a quarter ahead. '}
            {h.responded ? 'You answered it. ' : ''}
            {h.grant_credits !== null
              ? <strong>{windfallText(h.grant_credits, { landed: true })}</strong>
              : <><strong>{overallText(h.overall_ppm)}</strong>.</>}
          </p>
        </div>
      </li>)}
    </ul> : <p className="forecast-empty"><Icon name="banner" size={20} />
      <span>No headline has landed yet. They are invented, for the game.</span></p>}

    {snapshot.calendar.real_news.length > 0 && <>
      <h3 className="section-label report-gap">Real news this game has used</h3>
      <p className="hint">Real stories from GOV.UK, taken when each term began. A fixed list of keyword rules decides whether a story
        moves support and how; a story that matches none is shown here and changes nothing.</p>
      <ul className="ledger news-log real-news-log">
        {[...snapshot.calendar.real_news].reverse().map(n => <li key={n.url} className="ledger-item headline">
          <span className="tag">Term {n.term}</span>
          <div className="ledger-body">
            <div className="ledger-title"><strong>{n.title}</strong></div>
            <p className="ledger-votes">{n.summary}</p>
            <p className="ledger-votes">{dayText(n.published)}. {realNoteText(n)}{' '}
              <a href={n.url} target="_blank" rel="noreferrer noopener">Read the story on GOV.UK</a></p>
          </div>
        </li>)}
      </ul>
    </>}

    <h3 className="section-label report-gap">Progress reports from the ministers</h3>
    {snapshot.delivery_history.length ? (
      <>
        <div className="forecast-scroll">
          <table className="ledger-table">
            <caption className="sr-only">Units each area reports delivering, against the units due by then, after each quarter.
              These are the ministers’ own reports, not checked delivery.</caption>
            <thead>
              <tr>
                <th scope="col">Quarter</th>
                {FAMILIES.map(f => <th key={f.id} scope="col" className={`family-${f.id}`}>
                  <span className="th-family"><Icon name={f.id} size={16} />{f.label}</span>
                  <small>reported / due</small>
                </th>)}
                <th scope="col">Public support<small>game model</small></th>
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
        <p className="hint">Running totals as reported by the ministers. They are not checked, and can be optimistic. An audit shows the real figure.</p>
      </>
    ) : <p className="forecast-empty"><Icon name="hourglass" size={20} />
      <span>No quarter has finished yet. Approving a plan doesn’t deliver anything until time moves on.</span></p>}

    {(latest?.staff_slowdown_next || latest?.access_bonus_next) && <ul className="pressures report-gap">
      {latest.staff_slowdown_next && <li className="pressure pressure-risk">
        <Icon name="alert" size={16} /><span><strong>Risk next quarter</strong>
          The School Staff Association has called a slowdown for next quarter, so staffing may deliver less.</span></li>}
      {latest.access_bonus_next && <li className="pressure pressure-boost">
        <Icon name="spark" size={16} /><span><strong>Boost next quarter</strong>
          Community volunteers are adding access capacity.</span></li>}
    </ul>}

    {comparison.mode !== 'primary_campaign' && <>
      <h3 className="section-label report-gap">This is not your main game</h3>
      <ul className="chips">
        <li className="chip">{MODE_LABEL[comparison.mode]}</li>
        {comparison.source_campaign_id && <li className="chip">Branched from {comparison.source_campaign_id}</li>}
      </ul>
      {comparison.mode === 'fresh_scripted_branch' && <p className="hint report-hint">
        Ministers act afresh after the split, so comparing it with the original is not an exact replay.</p>}
    </>}

    <p className="panel-foot">Budget and reports come from the saved game. What the ministers privately chose to do,
      and what was really delivered, is not shown here. Nothing here predicts real UK school outcomes.</p>
  </section>
}

export function ConnectedEvidenceReport() {
  const {data,isError}=usePublicView()
  const snapshot=data?.snapshot
  if (!snapshot) return <Pending title="Case provenance" error={isError}/>
  const evidence = snapshot.evidence
  return <section className="panel" aria-labelledby="connected-evidence-report">
    <div className="panel-head">
      <h2 id="connected-evidence-report">Where this game’s data comes from</h2>
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

    <h3 className="section-label report-gap">News and scenario events</h3>
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

    <p className="panel-foot">Results in the game are simulation output. Real-world facts come from the archived sources
      above; story events are fictional. No AI wrote any of this text.</p>
    <p className="panel-foot">The fingerprints prove which authored inputs this game uses. They don’t verify the
      underlying facts or predict real outcomes.</p>
  </section>
}
