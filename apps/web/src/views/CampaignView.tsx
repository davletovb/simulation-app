import { ConnectedCampaignReport, usePublicView } from './ConnectedReports'
import { OFFLINE_PILOT } from '../contracts'
import {
  COHORT_CELLS, FACTIONS, FAMILIES, FIRST_FREE_AUDIT_QUARTER,
  QUARTERS, lastQuarterLabel,
} from '../game'
import { BlocMap } from '../components/BlocMap'
import { ElectionBars } from '../components/ElectionBars'
import { Icon } from '../components/Icon'
import { Mechanics } from '../components/Mechanics'
import { groups } from '../election.mjs'

/** Things that become possible, or change, in a given quarter (from the fixed delivery delays). */
const QUARTER_EVENTS: readonly string[][] = QUARTERS.map((_, i) => [
  ...FAMILIES.filter(f => f.lag === i).map(f => `${f.label} work can start`),
  ...(i + 1 === FIRST_FREE_AUDIT_QUARTER ? ['Free audit unlocks'] : []),
  ...(i === QUARTERS.length - 1 ? ['Campaign ends'] : []),
])

export function CampaignView() {
  const { data } = usePublicView()
  const snapshot = data?.snapshot
  const current = snapshot ? (snapshot.closed ? QUARTERS.length : snapshot.quarter) : 0
  return (
    <>
      <header className="view-head">
        <p className="kicker">{OFFLINE_PILOT.total_quarters} quarters · {QUARTERS[0]?.label} to {lastQuarterLabel()}</p>
        <h1>How the campaign is going</h1>
        <p>Work arrives late and in limited amounts, groups react a quarter after the fact, and unfinished promises stay
          on the record.</p>
      </header>

      <section className="panel" aria-labelledby="track-title">
        <div className="panel-head">
          <h2 id="track-title">The six quarters</h2>
          <span className="tag tag-calm">{snapshot?.closed ? 'Complete' : snapshot ? `You are in Q${snapshot.quarter}` : 'Not loaded'}</span>
        </div>
        <ol className="track">
          {QUARTERS.map((q, i) => {
            const state = i + 1 < current || (snapshot?.closed ?? false) ? 'is-done' : i + 1 === current ? 'is-next' : ''
            return (
              <li key={q.label} className={`track-node ${state}`}>
                <span className="track-dot" aria-hidden="true" />
                <strong>{q.short}<small>{q.year}</small></strong>
                {i + 1 === current && !snapshot?.closed && <span className="track-now">You are here</span>}
                <ul className="track-events">
                  {(QUARTER_EVENTS[i] ?? []).map(e => <li key={e}>{e}</li>)}
                </ul>
              </li>
            )
          })}
        </ol>
        <p className="panel-foot">Work starts after the delays above, counted from the quarter a plan is approved. These dates assume approval in Q1.</p>
      </section>

      <ConnectedCampaignReport />

      <div className="split">
        <section className="panel" aria-labelledby="factions-title">
          <div className="panel-head">
            <h2 id="factions-title">Organisations to keep an eye on</h2>
            <span className="tag tag-calm">What they want</span>
          </div>
          <div className="factions">
            {FACTIONS.map(f => (
              <article key={f.id} className={`faction faction-${f.id}`}>
                <h3><Icon name="banner" size={18} />{f.name}</h3>
                <div className="faction-meter" role="img" aria-label={`${f.resource}: ${f.start} of ${f.max} at the start`}>
                  {Array.from({ length: f.max }, (_, i) => (
                    <span key={i} className={i < f.start ? 'pip pip-on' : 'pip'} />
                  ))}
                </div>
                <p className="faction-resource">{f.resource} {f.start}/{f.max} at the start</p>
                <p>{f.interest}</p>
                <Mechanics>
                  <p>{f.rule}</p>
                  <p>{f.resource}: +{f.perQuarter} per quarter, up to {f.max}.</p>
                </Mechanics>
              </article>
            ))}
          </div>
          <p className="panel-foot">Members overlap the {COHORT_CELLS} groups of the public shown next; they are never extra people.</p>
        </section>

        <section className="panel" aria-labelledby="cohorts-title">
          <div className="panel-head">
            <h2 id="cohorts-title">The public you serve</h2>
            <span className="tag">{snapshot ? 'Live' : 'Not loaded'}</span>
          </div>
          {snapshot ? (
            <>
              <BlocMap groups={groups(snapshot, 'current')} showMood moodWhen={snapshot.closed ? 'on election night' : 'next quarter'} />
              <p className="panel-foot">Support as of the last finished quarter, starting from 50% for everyone. The change shown is
                already on its way and lands {snapshot.closed ? 'on election night' : 'next quarter'}. A group backs you above 50%.</p>
              <h3 className="section-label report-gap">If the election were held now</h3>
              <ElectionBars election={snapshot.election} />
            </>
          ) : <p className="forecast-empty"><Icon name="hourglass" size={20} /><span>Start or reconnect to the game to see how the public feels.</span></p>}
        </section>
      </div>
    </>
  )
}
