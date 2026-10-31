import { ConnectedCampaignReport } from './ConnectedReports'
import { OFFLINE_PILOT } from '../contracts'
import {
  COHORT_AREAS, COHORT_CELLS, COHORT_INCOMES, COHORT_START_SUPPORT, FACTIONS, FAMILIES, FIRST_FREE_AUDIT_QUARTER,
  PAID_AUDIT_COST, QUARTERS, lastQuarterLabel,
} from '../game'
import { Icon, type IconName } from '../components/Icon'
import { Mechanics } from '../components/Mechanics'

const QUARTER_EVENTS: readonly string[][] = QUARTERS.map((_, i) => [
  ...FAMILIES.filter(f => f.lag === i).map(f => `${f.label} can start`),
  ...(i + 1 === FIRST_FREE_AUDIT_QUARTER ? ['Free audit unlocks'] : []),
  ...(i === QUARTERS.length - 1 ? ['Campaign ends'] : []),
])

const ACTIONS: readonly { icon: IconName; name: string; detail: string }[] = [
  { icon: 'command', name: 'Propose or amend', detail: 'Fund new work from available credits. Needs a board vote.' },
  { icon: 'board', name: 'Change throughput caps', detail: 'Raise or lower public delivery ceilings. Needs a board vote.' },
  { icon: 'audit', name: 'Order an audit',
    detail: `Check what was really delivered. No vote; first one free from Q${FIRST_FREE_AUDIT_QUARTER}, then ${PAID_AUDIT_COST} credits.` },
  { icon: 'hourglass', name: 'Advance the quarter', detail: 'Work, reactions and deadlines resolve. Only you can advance.' },
]

export function CampaignView() {
  return (
    <>
      <header className="view-head">
        <p className="kicker">{OFFLINE_PILOT.total_quarters} quarters · {QUARTERS[0]?.label} to {lastQuarterLabel()}</p>
        <h1>The campaign ahead</h1>
        <p>Each quarter you make one set of decisions, then advance. Work arrives with delays, groups react a quarter
          later, and unfinished promises stay on the record.</p>
      </header>

      <section className="panel" aria-labelledby="track-title">
        <div className="panel-head">
          <h2 id="track-title">Quarter track</h2>
          <span className="tag">Illustrative plan</span>
        </div>
        <ol className="track">
          {QUARTERS.map((q, i) => (
            <li key={q.label} className={`track-node${i === 0 ? ' is-next' : ''}`}>
              <span className="track-dot" aria-hidden="true" />
              <strong>{q.short}<small>{q.year}</small></strong>
              {i === 0 && <span className="track-now">Reference start</span>}
              <ul className="track-events">
                {(QUARTER_EVENTS[i] ?? []).map(e => <li key={e}>{e}</li>)}
              </ul>
            </li>
          ))}
        </ol>
        <p className="panel-foot">Start quarters assume a package ratified before the first advance.</p>
      </section>

      <section aria-labelledby="actions-title">
        <h2 id="actions-title" className="section-label">Each quarter you can</h2>
        <div className="actions">
          {ACTIONS.map(a => (
            <div key={a.name} className="action">
              <Icon name={a.icon} size={22} />
              <strong>{a.name}</strong>
              <span>{a.detail}</span>
              <span className="action-lock"><Icon name="lock" size={13} />Use enabled actions in Command; other actions remain planned</span>
            </div>
          ))}
        </div>
      </section>

      <div className="split">
        <section className="panel" aria-labelledby="factions-title">
          <div className="panel-head">
            <h2 id="factions-title">Organisations</h2>
            <span className="tag tag-calm">Public interests</span>
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
          <p className="panel-foot">Members overlap the {COHORT_CELLS} groups below; they are never extra people.</p>
        </section>

        <section className="panel" aria-labelledby="cohorts-title">
          <div className="panel-head">
            <h2 id="cohorts-title">Who is served</h2>
            <span className="tag">Awaiting evidence</span>
          </div>
          <table className="cohorts">
            <caption className="sr-only">{COHORT_CELLS} school-service groups and their authored starting support</caption>
            <thead>
              <tr><th scope="col"><span className="sr-only">Area</span></th>
                {COHORT_INCOMES.map(i => <th key={i} scope="col">{i} income</th>)}</tr>
            </thead>
            <tbody>
              {COHORT_AREAS.map(area => (
                <tr key={area}>
                  <th scope="row">{area}</th>
                  {COHORT_INCOMES.map(income => (
                    <td key={income}>
                      <span className="cohort-value">{COHORT_START_SUPPORT}%</span>
                      <span className="cohort-note">start</span>
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
          <p className="panel-foot">{COHORT_START_SUPPORT}% starting support is an authored value, not a poll. Group
            sizes wait for verified P2 evidence.</p>
        </section>
      </div>

      <ConnectedCampaignReport />
      <section className="panel empty" aria-labelledby="results-title">
        <Icon name="people" size={30} />
        <h2 id="results-title">Illustrative reference panel</h2>
        <p>The connected report above shows committed game decisions and treasury state. This illustrative panel is not a live result.</p>
        <button type="button" className="btn btn-primary" disabled aria-describedby="advance-why">
          <Icon name="lock" size={18} />Advance quarter
        </button>
        <p id="advance-why" className="cta-why">Advance the actual saved campaign in the Command tab.</p>
      </section>
    </>
  )
}
