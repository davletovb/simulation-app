import type { PreviewResult } from '../usePreview'
import { QUARTERS, draftStatus, lastQuarterLabel, type Allocation } from '../game'
import { Icon } from './Icon'

/** Best-case delivery ceiling per family and quarter for the current draft. */
export function DeliveryForecast({ allocation, preview }: { allocation: Allocation; preview: PreviewResult }) {
  const status = draftStatus(allocation)
  if (status !== 'valid') {
    return (
      <div className="forecast-empty">
        <Icon name={status === 'empty' ? 'hourglass' : 'alert'} size={22} />
        <p>{status === 'empty'
          ? 'Deferring delivers nothing yet. You can still propose a funded package in a later quarter.'
          : 'Bring the package within budget to see when its work could arrive.'}</p>
      </div>
    )
  }

  if (!preview.rows) {
    return (
      <div className="forecast-empty" role="status">
        <Icon name="hourglass" size={22} />
        <p>{preview.status === 'loading'
          ? 'Calculating the optimistic delivery ceiling with the local Rust service…'
          : 'Rust delivery preview unavailable. Start the loopback service (cargo run -p myth-local-api) to see a verified estimate. No TypeScript fallback is substituted.'}</p>
      </div>
    )
  }
  const rows = preview.rows
  const shortfalls = rows.filter(r => r.outstanding > 0)
  return (
    <>
      <p className="panel-note" role="status">Optimistic ceiling · calculated by Rust · not a simulated result</p>
      <div className="forecast-scroll">
        <table className="forecast-table">
          <caption className="sr-only">
            Upper bound for each family if ratified before the first advance; optional access volunteer capacity included when attainable
          </caption>
          <thead>
            <tr>
              <th scope="col"><span className="sr-only">Family</span></th>
              {QUARTERS.map(q => (
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
                        ? <span className="cell-lock" aria-label="Not yet eligible"><Icon name="lock" size={13} /></span>
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
      {shortfalls.length > 0 && (
        <ul className="forecast-warnings">
          {shortfalls.map(r => (
            <li key={r.family.id}>
              <Icon name="alert" size={16} />
              {r.outstanding} {r.family.label.toLowerCase()} unit{r.outstanding === 1 ? '' : 's'} can’t be delivered
              by {lastQuarterLabel()}, even at full effort. They would stay as visible unfinished obligations.
            </li>
          ))}
        </ul>
      )}
    </>
  )
}
