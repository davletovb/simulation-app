import type { PreviewResult } from '../usePreview'
import { draftStatus, termEndLabel, termQuarters, type Allocation } from '../game'
import { Icon } from './Icon'

/** Best-case delivery ceiling per family and quarter for the current plan. */
export function DeliveryForecast({ allocation, preview, term, budget }: {
  allocation: Allocation
  preview: PreviewResult
  /** The term the plan would run in: its six quarters are the columns. */
  term: number
  /** What the player can fund now. */
  budget: number
}) {
  const status = draftStatus(allocation, budget)
  if (status !== 'valid') {
    return (
      <div className="forecast-empty">
        <Icon name={status === 'empty' ? 'hourglass' : 'alert'} size={22} />
        <p>{status === 'empty'
          ? 'Build a plan to see when its work could arrive.'
          : 'Bring the plan within budget to see when its work could arrive.'}</p>
      </div>
    )
  }

  if (!preview.rows) {
    return (
      <div className="forecast-empty" role="status">
        <Icon name="hourglass" size={22} />
        <p>{preview.status === 'loading'
          ? 'Working out the best case…'
          : 'The forecast isn’t available because the game service isn’t responding.'}</p>
        {preview.status === 'unavailable' && (
          <button type="button" className="btn btn-ghost btn-sm" onClick={preview.retry}>Try again</button>
        )}
      </div>
    )
  }
  const rows = preview.rows
  const shortfalls = rows.filter(r => r.outstanding > 0)
  return (
    <>
      <p className="panel-note" role="status">
        {preview.status === 'updating'
          ? 'Updating… the figures below still describe your previous plan.'
          : 'Best case: every minister works flat out, with no strikes or shocks.'}
      </p>
      <div className="forecast-scroll">
        <table className="forecast-table">
          <caption className="sr-only">
            Most units each area could deliver per quarter if this plan were approved now
          </caption>
          <thead>
            <tr>
              <th scope="col"><span className="sr-only">Area</span></th>
              {termQuarters(term).map(q => (
                <th scope="col" key={q.label}>{q.short}<small>{q.year}</small></th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map(({ family, perQuarter }) => (
              <tr key={family.id} className={`row-${family.id}`}>
                <th scope="row">
                  <span className="row-head">
                    <Icon name={family.id} size={18} />
                    <span className="row-name">{family.label}</span>
                    <small>{allocation[family.id]}<span className="funded-word"> funded</span></small>
                  </span>
                </th>
                {perQuarter.map((units, i) => {
                  const locked = i < family.lag
                  return (
                    <td key={i} className={locked ? 'cell-locked' : units === 0 ? 'cell-idle' : undefined}>
                      {locked
                        ? <span className="cell-lock" aria-label="Not started yet"><Icon name="lock" size={13} /></span>
                        : <span className="cell-bar" style={{ height: `${(units / (family.quarterCap + (family.id === 'access' && allocation.access >= 10 ? 1 : 0))) * 100}%` }}>
                            <span className="cell-units">{units}</span>
                          </span>}
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {preview.status === 'ready' && shortfalls.length > 0 && (
        <ul className="forecast-warnings">
          {shortfalls.map(r => (
            <li key={r.family.id}>
              <Icon name="alert" size={16} />
              {r.outstanding} {r.family.label.toLowerCase()} unit{r.outstanding === 1 ? '' : 's'} can’t be delivered
              by {termEndLabel(term)}, even in the best case. They would stay on the record as unfinished promises.
            </li>
          ))}
        </ul>
      )}
    </>
  )
}
