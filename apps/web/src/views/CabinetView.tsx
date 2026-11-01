import { CabinetMeetingStatus, SeatPositions, usePublicView } from './ConnectedReports'
import { finalSupport } from '../positions.mjs'
import { FAMILIES, MINISTERS, STEP_HELP, VOTES_NEEDED } from '../game'
import { STEPS } from '../turn.mjs'
import { Icon } from '../components/Icon'

export function CabinetView() {
  const { data, isError } = usePublicView()
  const snapshot = data?.snapshot
  const support = snapshot ? finalSupport(snapshot.votes) : 0
  return (
    <>
      <header className="view-head">
        <p className="kicker">Your cabinet</p>
        <h1>The three ministers who vote on your plans</h1>
        <p>You chair the cabinet. You propose the plans and make the final call, but you can’t override a vote, the
          budget, or how hard a minister privately chooses to work.</p>
      </header>

      <CabinetMeetingStatus snapshot={snapshot} unavailable={isError} />
      <section className="council" aria-label="The ministers">
        <div className="seats">
          {MINISTERS.map(minister => {
            const owned = FAMILIES.find(f => f.id === minister.owns)
            return (
              <article key={minister.id} className={`seat seat-${minister.owns ?? 'finance'}`}>
                <div className="seat-emblem" aria-hidden="true">{minister.name[0]}</div>
                <h2>{minister.name}</h2>
                <p className="seat-title">{minister.role}</p>
                <p className="seat-mandate">{minister.duty}</p>
                {owned && (
                  <ul className="chips" aria-label={`What you can’t see about the ${minister.name} minister`}>
                    <li className="chip chip-private">
                      <Icon name="eye-off" size={14} />How hard they push {owned.label} is hidden from you
                    </li>
                  </ul>
                )}
                <SeatPositions ministry={minister.id} seat={minister.name} snapshot={snapshot} unavailable={isError} />
              </article>
            )
          })}
        </div>

        <div className="council-table">
          <span className="table-label">The rule</span>
          <strong>A plan needs {VOTES_NEEDED} of 3 ministers to support it</strong>
          <span>Any conditions a minister attaches must also be funded and achievable within the campaign.</span>
          <div className="tally" role="img" aria-label={`${support} of 3 ministers support the final plan; ${VOTES_NEEDED} needed`}>
            {MINISTERS.map((m, i) => <span key={m.id} className={`tally-pip${i < support ? ' is-on' : ''}`} />)}
            <em>{support} support · {VOTES_NEEDED} needed</em>
          </div>
        </div>

        <article className="chair">
          <div className="seat-emblem chair-emblem" aria-hidden="true"><Icon name="chair" size={22} /></div>
          <div>
            <h2>You · Chair</h2>
            <p>Propose a plan, revise it once, and approve it if it passes. You see the ministers’ public reports and
              the results of your own audits, never how hard a minister privately works.</p>
          </div>
        </article>
      </section>

      <section className="panel" aria-labelledby="phases-title">
        <div className="panel-head">
          <h2 id="phases-title">How a quarter works</h2>
          <span className="tag tag-calm">The same six steps every quarter</span>
        </div>
        <ol className="phases">
          {STEPS.map((step, i) => (
            <li key={step.id} className="phase">
              <span className="phase-num">{i + 1}</span>
              <strong>{step.label}</strong>
              <span>{STEP_HELP[step.id]}</span>
            </li>
          ))}
        </ol>
        <p className="panel-foot">Talking with the cabinet never uses up a quarter. Changes you miss wait for the next quarter’s meeting.</p>
      </section>
    </>
  )
}
