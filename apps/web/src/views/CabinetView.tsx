import { CabinetMeetingStatus, SeatPositions, usePublicView } from './ConnectedReports'
import { finalSupport } from '../positions.mjs'
import { FAMILIES, MEETING_PHASES, SEATS, VOTES_NEEDED } from '../game'
import { Icon } from '../components/Icon'

export function CabinetView() {
  const { data, isError } = usePublicView()
  const snapshot = data?.snapshot
  const support = snapshot ? finalSupport(snapshot.votes) : 0
  return (
    <>
      <header className="view-head">
        <p className="kicker">Your board</p>
        <h1>The Education Delivery Board</h1>
        <p>Three fictional seats vote on every package. You chair the meeting, but you can’t override a vote, the
          budget or a seat’s private choices.</p>
      </header>

      <CabinetMeetingStatus snapshot={snapshot} unavailable={isError} />
      <section className="council" aria-label="Board seats">
        <div className="seats">
          {SEATS.map(seat => {
            const owned = FAMILIES.find(f => f.id === seat.owns)
            return (
              <article key={seat.name} className={`seat seat-${seat.owns ?? 'finance'}`}>
                <div className="seat-emblem" aria-hidden="true">{seat.name[0]}</div>
                <h2>{seat.name}</h2>
                <p className="seat-title">{seat.title}</p>
                <p className="seat-mandate">{seat.mandate}</p>
                <ul className="chips" aria-label={`${seat.name} powers`}>
                  {seat.powers.map(p => <li key={p} className="chip">{p}</li>)}
                  {owned && (
                    <li className="chip chip-private">
                      <Icon name="eye-off" size={14} />{owned.label} effort hidden from you
                    </li>
                  )}
                </ul>
                <SeatPositions ministry={seat.name.toLowerCase()} seat={seat.name} snapshot={snapshot} unavailable={isError} />
              </article>
            )
          })}
        </div>

        <div className="council-table">
          <span className="table-label">Approval rule</span>
          <strong>{VOTES_NEEDED} of 3 seats must support the same frozen version</strong>
          <span>Conditions must also be funded, observable and due within the campaign.</span>
          <div className="tally" role="img" aria-label={`${support} of 3 seats support; ${VOTES_NEEDED} needed`}>
            {SEATS.map((s, i) => <span key={s.name} className={`tally-pip${i < support ? ' is-on' : ''}`} />)}
            <em>{support} support · {VOTES_NEEDED} needed</em>
          </div>
        </div>

        <article className="chair">
          <div className="seat-emblem chair-emblem" aria-hidden="true"><Icon name="chair" size={22} /></div>
          <div>
            <h2>You · Chair</h2>
            <p>Propose a package, amend it once per meeting and ratify it if it passes. You see public reports and
              your own audits, never a seat’s private effort.</p>
          </div>
        </article>
      </section>

      <section className="panel" aria-labelledby="phases-title">
        <div className="panel-head">
          <h2 id="phases-title">How a meeting runs</h2>
          <span className="tag tag-calm">One meeting per quarter</span>
        </div>
        <ol className="phases">
          {MEETING_PHASES.map((phase, i) => (
            <li key={phase.label} className="phase">
              <span className="phase-num">{i + 1}</span>
              <strong>{phase.label}</strong>
              <span>{phase.detail}</span>
            </li>
          ))}
        </ol>
        <p className="panel-foot">Negotiating never uses up a quarter. Late changes wait for next quarter’s meeting.</p>
      </section>
    </>
  )
}
