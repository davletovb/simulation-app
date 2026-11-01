import { ConnectedCampaignReport, usePublicView } from './ConnectedReports'
import { OFFLINE_PILOT } from '../contracts'
import {
  COHORT_CELLS, FACTIONS, FAMILIES, FIRST_FREE_AUDIT_QUARTER,
  QUARTERS, lastQuarterLabel, termEndLabel, termQuarters,
} from '../game'
import { BlocMap } from '../components/BlocMap'
import { ElectionBars } from '../components/ElectionBars'
import { Icon } from '../components/Icon'
import { Mechanics } from '../components/Mechanics'
import { groups, termName } from '../election.mjs'

const TERMS = Array.from({ length: OFFLINE_PILOT.terms }, (_, i) => i + 1)

/**
 * Things that become possible, or change, in a given quarter of a term (from the fixed delivery delays).
 * Every term has the same shape: work starts after the same delays, the free audit unlocks again, and
 * the term ends with an election.
 */
const termEvents = (term: number): readonly string[][] => termQuarters(term).map((_, i) => [
  ...FAMILIES.filter(f => f.lag === i).map(f => `${f.label} work can start`),
  ...(i + 1 === FIRST_FREE_AUDIT_QUARTER ? ['Free audit unlocks'] : []),
  ...(i === OFFLINE_PILOT.quarters_per_term - 1
    ? [term === OFFLINE_PILOT.terms ? 'Final election' : 'Election: win to start the next term'] : []),
])

export function CampaignView() {
  const { data } = usePublicView()
  const snapshot = data?.snapshot
  const night = snapshot?.phase === 'ElectionNight'
  const current = snapshot ? (snapshot.closed ? QUARTERS.length : snapshot.quarter) : 0
  return (
    <>
      <header className="view-head">
        <p className="kicker">{OFFLINE_PILOT.terms} terms · {OFFLINE_PILOT.total_quarters} quarters · {QUARTERS[0]?.label} to {lastQuarterLabel()}</p>
        <h1>How the campaign is going</h1>
        <p>Work arrives late and in limited amounts, groups react a quarter after the fact, and unfinished promises stay
          on the record. Each term ends with an election.</p>
      </header>

      <section className="panel" aria-labelledby="track-title">
        <div className="panel-head">
          <h2 id="track-title">The two terms</h2>
          <span className="tag tag-calm">{snapshot?.closed ? 'Complete'
            : night ? `Term ${snapshot.term} won`
            : snapshot ? `You are in term ${snapshot.term}, Q${snapshot.quarter}` : 'Not loaded'}</span>
        </div>
        {TERMS.map(term => (
          <div key={term} className="term-block">
            <h3 className="section-label">{termName(term).replace(/^./, c => c.toUpperCase())} term · election after {termEndLabel(term)}</h3>
            <ol className="track">
              {termQuarters(term).map((q, i) => {
                const n = (term - 1) * OFFLINE_PILOT.quarters_per_term + i + 1
                const state = (snapshot?.closed ?? false) || n < current || (night && n === current) ? 'is-done'
                  : n === current ? 'is-next' : ''
                return (
                  <li key={q.label} className={`track-node ${state}`}>
                    <span className="track-dot" aria-hidden="true" />
                    <strong>{q.short}<small>{q.year}</small></strong>
                    {n === current && !snapshot?.closed && !night && <span className="track-now">You are here</span>}
                    <ul className="track-events">
                      {(termEvents(term)[i] ?? []).map(e => <li key={e}>{e}</li>)}
                    </ul>
                  </li>
                )
              })}
            </ol>
          </div>
        ))}
        <p className="panel-foot">Work starts after the delays above, counted from the quarter a plan is approved. These dates assume
          approval in the first quarter of a term. A second term starts with a fresh {OFFLINE_PILOT.credits} credits.</p>
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
              <BlocMap groups={groups(snapshot, 'current')} showMood moodWhen={snapshot.closed || night ? 'on election night' : 'next quarter'} />
              <p className="panel-foot">Support as of the last finished quarter, starting from 50% for everyone. The change shown is
                already on its way and lands {snapshot.closed || night ? 'on election night' : 'next quarter'}. A group backs you above 50%.</p>
              <h3 className="section-label report-gap">{snapshot.closed || night ? 'The result' : 'If the election were held now'}</h3>
              <ElectionBars election={snapshot.election} />
            </>
          ) : <p className="forecast-empty"><Icon name="hourglass" size={20} /><span>Start or reconnect to the game to see how the public feels.</span></p>}
        </section>
      </div>
    </>
  )
}
