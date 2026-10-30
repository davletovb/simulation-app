import { useRef, useState, type CSSProperties, type KeyboardEvent } from 'react'
import { OFFLINE_PILOT } from '../contracts'
import {
  DEFAULT_CARD, FACTIONS, FAMILIES, POLICY_CARDS, QUARTERS, committed, draftStatus,
  lastQuarterLabel, sameAllocation, type Allocation, type Family, type FamilyInfo, type PolicyCard,
} from '../game'
import { CreditMeter } from '../components/CreditMeter'
import { DeliveryForecast } from '../components/DeliveryForecast'
import { usePreview, type PreviewResult } from '../usePreview'
import { Icon } from '../components/Icon'
import { Mechanics } from '../components/Mechanics'

type CardChoice = PolicyCard | 'custom'

function PolicyHand({ allocation, onPick }: {
  allocation: Allocation
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
        const over = cardAllocation !== null && committed(cardAllocation) > OFFLINE_PILOT.credits
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
            {over && <span className="card-flag">Over budget</span>}
            {!isCustom && choice.id === 'defer' && <span className="card-flag card-flag-calm">No reform</span>}
            <span className="card-name">{isCustom ? 'Your own split' : choice.name}</span>
            <span className="card-tagline">
              {isCustom ? 'Choose this card, then tune the sliders in the draft' : choice.tagline}
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

function FamilyControl({ family, value, onChange }: {
  family: FamilyInfo
  value: number
  onChange: (next: number) => void
}) {
  const max = OFFLINE_PILOT.credits
  const set = (next: number) => onChange(Math.min(max, Math.max(0, next)))
  const firstQuarter = QUARTERS[family.lag]?.label ?? 'later'
  return (
    <div className={`family family-${family.id}`}>
      <div className="family-head">
        <label htmlFor={`range-${family.id}`}><Icon name={family.id} size={18} />{family.label}</label>
        <output htmlFor={`range-${family.id}`} className="family-value">{value}</output>
      </div>
      <div className="family-control">
        <button type="button" className="step" onClick={() => set(value - 1)} disabled={value === 0}
          aria-label={`One less ${family.label.toLowerCase()} credit`}><Icon name="minus" size={16} /></button>
        <input id={`range-${family.id}`} type="range" min={0} max={max} step={1} value={value}
          onChange={event => set(Number(event.target.value))} aria-describedby={`meta-${family.id}`}
          style={{ '--fill': `${(value / max) * 100}%` } as CSSProperties} />
        <button type="button" className="step" onClick={() => set(value + 1)} disabled={value === max}
          aria-label={`One more ${family.label.toLowerCase()} credit`}><Icon name="plus" size={16} /></button>
      </div>
      <p id={`meta-${family.id}`} className="family-meta">
        1 credit = 1 {family.unit} · up to {family.quarterCap} per quarter · first delivery {firstQuarter}
      </p>
    </div>
  )
}

function Pressures({ allocation, preview }: { allocation: Allocation; preview: PreviewResult }) {
  const reactions = FACTIONS.flatMap(f => {
    const pressure = f.pressure(allocation)
    return pressure ? [{ faction: f, pressure }] : []
  })
  const fresh = preview.status === 'ready'
  const shortfall = fresh ? preview.rows?.reduce((sum, row) => sum + row.outstanding, 0) ?? 0 : 0
  const pending = preview.status === 'loading' || preview.status === 'updating' || preview.status === 'unavailable'
  if (reactions.length === 0 && shortfall === 0 && !pending) return null
  return (
    <ul className="pressures" aria-label="Likely reactions to this draft">
      {reactions.map(({ faction, pressure }) => (
        <li key={faction.id} className={`pressure pressure-${pressure.kind}`}>
          <Icon name={pressure.kind === 'risk' ? 'alert' : 'spark'} size={16} />
          <span>
            <strong>{faction.name}</strong>{pressure.text}
            <Mechanics><p>{faction.rule}</p></Mechanics>
          </span>
        </li>
      ))}
      {pending && (
        <li className="pressure pressure-neutral" role="status">
          <Icon name="hourglass" size={16} />
          <span><strong>Delivery horizon</strong>
            {preview.status === 'unavailable'
              ? 'Rust preview unavailable. The delivery shortfall is unknown until the local service responds.'
              : 'Checking the delivery horizon with Rust. The current draft has no verified forecast yet.'}
          </span>
        </li>
      )}
      {fresh && shortfall > 0 && (
        <li className="pressure pressure-risk">
          <Icon name="hourglass" size={16} />
          <span><strong>Delivery horizon</strong>{shortfall} funded unit{shortfall === 1 ? '' : 's'} can’t arrive
            by {lastQuarterLabel()} even at full effort.</span>
        </li>
      )}
    </ul>
  )
}

export function CommandView({ allocation, onChange }: {
  allocation: Allocation
  onChange: (next: Allocation) => void
}) {
  const setFamily = (family: Family, value: number) => onChange({ ...allocation, [family]: value })
  const preview = usePreview(allocation)

  return (
    <>
      <header className="view-head">
        <p className="kicker">{QUARTERS[0]?.label} · Opening move</p>
        <h1>Choose your opening package</h1>
        <p>Play a policy card or build your own split of the {OFFLINE_PILOT.credits} pilot credits. They must last
          all {OFFLINE_PILOT.total_quarters} quarters, so anything you hold back can fund later packages or audits.</p>
      </header>

      <div className="command-layout">
        <section className="panel hand-panel" aria-labelledby="hand-title">
          <div className="panel-head">
            <h2 id="hand-title">Policy cards</h2>
            <span className="hint">Arrow keys to browse</span>
          </div>
          <PolicyHand allocation={allocation} onPick={card => onChange(card.allocation)} />
        </section>

        <section className="panel draft" aria-labelledby="draft-title">
          <div className="panel-head">
            <h2 id="draft-title">Draft package</h2>
            <span className="tag">Not submitted</span>
          </div>
          <CreditMeter allocation={allocation} />
          <div className="families">
            {FAMILIES.map(f => (
              <FamilyControl key={f.id} family={f} value={allocation[f.id]} onChange={v => setFamily(f.id, v)} />
            ))}
          </div>
          <Pressures allocation={allocation} preview={preview} />
          <div className="cta-row">
            <button type="button" className="btn btn-primary" disabled aria-describedby="cta-why">
              <Icon name="lock" size={18} />Convene the board
            </button>
            <button type="button" className="btn btn-ghost" onClick={() => onChange(DEFAULT_CARD.allocation)}
              disabled={sameAllocation(allocation, DEFAULT_CARD.allocation)}>
              <Icon name="reset" size={18} />Reset
            </button>
          </div>
          <p id="cta-why" className="cta-why">
            Real cabinet votes require the full simulation. The local service supplies scripted baseline snapshots only, without ratification.
          </p>
        </section>

        <section className="panel forecast" aria-labelledby="forecast-title">
          <div className="panel-head">
            <h2 id="forecast-title">Best-case delivery</h2>
            <span className="tag tag-calm">Illustrative ceiling</span>
          </div>
          <p className="panel-lede">The most each family could deliver if this package were ratified before the first
            advance, with full effort and no slowdowns or shocks. For access funding of 10 or more, the optimistic ceiling includes the Community Access Network’s attainable +1 capacity from Q2 onward, which requires its volunteer action. It is not guaranteed; real delivery may be lower.
            The local Rust service calculates this conditional optimistic ceiling. Actual outcomes still require the P3 simulation engine.</p>
          <DeliveryForecast allocation={allocation} preview={preview} />
        </section>
      </div>
    </>
  )
}
