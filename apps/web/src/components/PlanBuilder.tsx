import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react'
import {
  FACTIONS, FAMILIES, POLICY_CARDS, committed, firstDeliveryLabel, lastQuarterLabel, sameAllocation,
  type Allocation, type Family, type FamilyInfo, type PolicyCard,
} from '../game'
import { CreditMeter } from './CreditMeter'
import { DeliveryForecast } from './DeliveryForecast'
import { Icon } from './Icon'
import { Mechanics } from './Mechanics'
import { usePreview, type PreviewResult } from '../usePreview'
import { parseExactBudget } from '../exactBudget.mjs'

type CardChoice = PolicyCard | 'custom'

function ReadyMadePlans({ allocation, budget, onPick }: {
  allocation: Allocation
  budget: number
  onPick: (card: PolicyCard) => void
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([])
  const [customPicked, setCustomPicked] = useState(false)
  const matched = POLICY_CARDS.find(card => sameAllocation(card.allocation, allocation))
  const choices: readonly CardChoice[] = [...POLICY_CARDS, 'custom']
  const checkedIndex = matched && !customPicked ? POLICY_CARDS.indexOf(matched) : choices.length - 1

  const choose = (index: number) => {
    const choice = choices[index]
    if (!choice) return
    setCustomPicked(choice === 'custom')
    if (choice !== 'custom') onPick(choice)
    refs.current[index]?.focus()
  }

  // Radio-group keyboard pattern: arrows move and select, wrapping at the ends.
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key]
    if (step === undefined) return
    event.preventDefault()
    choose((index + step + choices.length) % choices.length)
  }

  return (
    <div className="hand" role="radiogroup" aria-labelledby="hand-title">
      {choices.map((choice, index) => {
        const checked = index === checkedIndex
        const isCustom = choice === 'custom'
        // The custom card mirrors the draft only while it is the active choice.
        const cardAllocation = !isCustom ? choice.allocation : checked ? allocation : null
        const over = cardAllocation !== null && committed(cardAllocation) > budget
        return (
          <button key={isCustom ? 'custom' : choice.id} ref={el => { refs.current[index] = el }}
            type="button" role="radio" aria-checked={checked} tabIndex={checked ? 0 : -1}
            className={`policy-card${checked ? ' is-selected' : ''}${over ? ' is-invalid' : ''}${isCustom ? ' is-custom' : ''}`}
            onClick={() => choose(index)} onKeyDown={event => onKeyDown(event, index)}>
            {cardAllocation && (
              <span className="card-cost" aria-label={`${committed(cardAllocation)} credits`}>
                <Icon name="token" size={14} />{committed(cardAllocation)}
              </span>
            )}
            {over && <span className="card-flag">Can’t afford</span>}
            <span className="card-name">{isCustom ? 'Your own split' : choice.name}</span>
            <span className="card-tagline">
              {isCustom ? 'Set each amount yourself with the sliders below' : choice.tagline}
            </span>
            {cardAllocation && (
              <span className="card-stats">
                {FAMILIES.map(f => (
                  <span key={f.id} className={`stat stat-${f.id}`}>
                    <Icon name={f.id} size={14} /><span className="sr-only">{f.label}</span>{cardAllocation[f.id]}
                  </span>
                ))}
              </span>
            )}
          </button>
        )
      })}
    </div>
  )
}

function FamilyControl({ family, value, budget, quarter, onChange }: {
  family: FamilyInfo
  value: number
  budget: number
  quarter: number
  onChange: (next: number) => void
}) {
  const max = budget
  const [draft, setDraft] = useState(() => String(value))
  const [invalid, setInvalid] = useState(false)
  useEffect(() => { setDraft(String(value)); setInvalid(false) }, [value])
  const set = (next: number) => {
    if (!Number.isSafeInteger(next)) return
    onChange(Math.min(max, Math.max(0, next)))
  }
  const acceptExact = () => {
    const parsed = parseExactBudget(draft, max)
    if (parsed === null) { setInvalid(true); return }
    setInvalid(false)
    set(parsed)
  }
  const starts = firstDeliveryLabel(family, quarter)
  return (
    <div className={`family family-${family.id}`}>
      <div className="family-head">
        <label htmlFor={`range-${family.id}`}><Icon name={family.id} size={18} />{family.label}</label>
        <output htmlFor={`range-${family.id}`} className="family-value">{value}</output>
      </div>
      <p className="family-blurb">{family.blurb}</p>
      <div className="family-control">
        <input type="text" inputMode="numeric" pattern="[0-9]{1,3}"
          aria-label={`${family.label}: exact number of credits`}
          aria-invalid={invalid} aria-describedby={`meta-${family.id}`}
          className="credit-input" value={draft}
          onChange={event => { setDraft(event.target.value); setInvalid(false) }}
          onBlur={acceptExact}
          onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); acceptExact() } }}
        />
        <button type="button" className="step" onClick={() => set(value - 1)} disabled={value === 0}
          aria-label={`One less credit for ${family.label.toLowerCase()}`}><Icon name="minus" size={16} /></button>
        <input id={`range-${family.id}`} type="range" min={0} max={max} step={1} value={value}
          onChange={event => set(Number(event.target.value))} aria-describedby={`meta-${family.id}`}
          style={{ '--fill': `${max > 0 ? (value / max) * 100 : 0}%` } as CSSProperties} />
        <button type="button" className="step" onClick={() => set(value + 1)} disabled={value === max}
          aria-label={`One more credit for ${family.label.toLowerCase()}`}><Icon name="plus" size={16} /></button>
      </div>
      {invalid && <p role="alert" className="field-error">Use a whole number of credits from 0 to {max}.</p>}
      <p id={`meta-${family.id}`} className="family-meta">
        1 credit buys 1 {family.unit} · at most {family.quarterCap} a quarter ·{' '}
        {starts
          ? <>work can start arriving in {starts}</>
          : <strong className="late">too late: it wouldn’t arrive before the campaign ends</strong>}
      </p>
    </div>
  )
}

/** What the organisations would think of this plan, plus any work that cannot arrive in time. */
function Reactions({ allocation, preview }: { allocation: Allocation; preview: PreviewResult | null }) {
  const reactions = FACTIONS.flatMap(f => {
    const pressure = f.pressure(allocation)
    return pressure ? [{ faction: f, pressure }] : []
  })
  const shortfall = preview?.status === 'ready' ? preview.rows?.reduce((sum, row) => sum + row.outstanding, 0) ?? 0 : 0
  if (reactions.length === 0 && shortfall === 0) return null
  return (
    <ul className="pressures" aria-label="Likely reactions to this plan">
      {reactions.map(({ faction, pressure }) => (
        <li key={faction.id} className={`pressure pressure-${pressure.kind}`}>
          <Icon name={pressure.kind === 'risk' ? 'alert' : 'spark'} size={16} />
          <span>
            <strong>{faction.name}</strong>{pressure.text}
            <Mechanics><p>{faction.rule}</p></Mechanics>
          </span>
        </li>
      ))}
      {shortfall > 0 && (
        <li className="pressure pressure-risk">
          <Icon name="hourglass" size={16} />
          <span><strong>Some of this can’t arrive in time</strong>{shortfall} funded unit{shortfall === 1 ? '' : 's'} would
            still be undelivered by {lastQuarterLabel()}, even in the best case.</span>
        </li>
      )}
    </ul>
  )
}

/**
 * Everything needed to make or revise a spending plan, in one block:
 * ready-made plans, sliders, the budget meter, who might react, and when the
 * work could arrive. Used by the "make a plan" and "revise" steps.
 */
export function PlanBuilder({ allocation, budget, quarter, onChange }: {
  allocation: Allocation
  budget: number
  quarter: number
  onChange: (next: Allocation) => void
}) {
  const setFamily = (family: Family, value: number) => onChange({ ...allocation, [family]: value })
  const preview = usePreview(allocation)
  // The service forecasts a plan approved at the start of the campaign, so it is
  // only a fair picture in Q1. Later quarters rely on the per-area start dates.
  const forecastable = quarter === 1
  return (
    <div className="builder">
      <section aria-labelledby="hand-title">
        <div className="panel-head">
          <h2 id="hand-title">Ready-made plans</h2>
          <span className="hint">Or build your own below</span>
        </div>
        <ReadyMadePlans allocation={allocation} budget={budget} onPick={card => onChange(card.allocation)} />
      </section>

      <section className="builder-split" aria-labelledby="split-title">
        <div className="panel-head">
          <h2 id="split-title">Your split</h2>
        </div>
        <CreditMeter allocation={allocation} budget={budget} />
        <div className="families">
          {FAMILIES.map(f => (
            <FamilyControl key={f.id} family={f} value={allocation[f.id]} budget={budget} quarter={quarter}
              onChange={v => setFamily(f.id, v)} />
          ))}
        </div>
        <Reactions allocation={allocation} preview={forecastable ? preview : null} />
      </section>

      {forecastable && (
        <details className="forecast-details">
          <summary>When could this plan’s work arrive?</summary>
          <div className="forecast-body">
            <DeliveryForecast allocation={allocation} preview={preview} />
          </div>
        </details>
      )}
    </div>
  )
}
