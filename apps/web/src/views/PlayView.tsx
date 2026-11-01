import { useEffect, useState, type ReactNode } from 'react'
import { CAMPAIGN, useCampaign, type AuditFamily, type Snapshot } from '../campaign'
import { OFFLINE_PILOT } from '../contracts'
import {
  FAMILIES, POLICY_CARDS, QUARTERS, STEP_HELP, VOTES_NEEDED, committed, draftStatus, sameAllocation,
  type Allocation,
} from '../game'
import { outcomeLabel } from '../format'
import { STEPS, advanceKind, currentProgramme, fundableCredits, turnState, undeliveredCredits, type TurnState } from '../turn.mjs'
import { ActionButton } from '../components/ConnectedControls'
import { BudgetFigures } from '../components/BudgetFigures'
import { CabinetStrip } from '../components/CabinetStrip'
import { Icon } from '../components/Icon'
import { PlanBuilder } from '../components/PlanBuilder'
import { StepTracker } from '../components/StepTracker'
import { TechnicalDetails } from '../components/TechnicalDetails'

const NONE: Allocation = { staff: 0, repairs: 0, access: 0 }

/** The plan on the table, read-only. */
function PlanSummary({ plan, label }: { plan: Allocation; label: string }) {
  return (
    <div className="plan-summary">
      <span className="room-label">{label}</span>
      <div className="card-stats room-stats">
        {FAMILIES.map(f => (
          <span key={f.id} className={`stat stat-${f.id}`}>
            <Icon name={f.id} size={14} />{f.label} {plan[f.id]}
          </span>
        ))}
        <span className="plan-total">= {committed(plan)} credits</span>
      </div>
    </div>
  )
}

/** Shown instead of the game while there is no saved campaign to play. */
function Gate() {
  const { loadState, createCampaign, refresh, error, staleSave } = useCampaign()
  const problem = error && <p role="alert" className="alert alert-danger"><Icon name="alert" size={16} /><span>{error}</span></p>
  // A fresh, valid name: lowercase letters, digits and hyphens only (the same rule the service enforces).
  const freshName = `${CAMPAIGN.slice(0, 40)}-${Date.now().toString(36)}`
  if (loadState === 'loading') {
    return (
      <section className="panel gate" role="status">
        <Icon name="hourglass" size={26} />
        <h1>Loading your campaign…</h1>
      </section>
    )
  }
  if (loadState === 'missing') {
    return (
      <section className="panel gate">
        <Icon name="banner" size={26} />
        <h1>Start your campaign</h1>
        <p>You will chair a cabinet for {OFFLINE_PILOT.total_quarters} quarters with {OFFLINE_PILOT.credits} credits to
          spend. Your progress is saved on this computer.</p>
        {problem}
        <div className="room-buttons">
          <ActionButton tone={staleSave ? 'normal' : 'primary'} onClick={() => void createCampaign()}>Start new campaign</ActionButton>
          {staleSave && <ActionButton tone="primary" onClick={() => { window.location.search = `?campaign=${freshName}` }}>
            Start fresh under a new name</ActionButton>}
        </div>
      </section>
    )
  }
  return (
    <section className="panel gate" role="alert">
      <Icon name="alert" size={26} />
      <h1>Can’t reach the game</h1>
      <p>The game runs as a small service on this computer, and it isn’t answering. Start it, then try again:</p>
      <pre className="gate-cmd"><code>cargo +1.99.1 run --locked -p myth-local-api</code></pre>
      {problem}
      <ActionButton tone="primary" onClick={refresh}>Try again</ActionButton>
    </section>
  )
}

function Headline({ snapshot, turn }: { snapshot: Snapshot; turn: TurnState }) {
  const q = snapshot.quarter
  const available = snapshot.treasury.available_credits
  const undelivered = undeliveredCredits(snapshot.treasury)
  const fundable = fundableCredits(snapshot.treasury)
  const plan = snapshot.proposal ? committed(snapshot.proposal) : 0
  const copy: Record<string, { title: string; lede: string }> = {
    plan: {
      title: q === 1 ? 'Make your first spending plan' : `Make the plan for quarter ${q}`,
      lede: fundable === 0
        ? 'Everything you funded has been delivered and nothing is left to spend, so there is nothing to plan. Carry on to the end of the campaign.'
        : q === 1
          ? `You have ${available} credits for the whole campaign, and they never refill. Pick a ready-made plan or set your own split, then propose it to your cabinet.`
          : `You can plan with up to ${fundable} credits: ${available} unspent, plus ${undelivered} for work that hasn’t been delivered yet. A new plan replaces that undelivered work; work already delivered stays. Or keep the current programme and skip the vote.`,
    },
    reactions: {
      title: 'Hear the cabinet’s first reactions',
      lede: 'Your plan is on the table. Next, each minister says whether they support it.',
    },
    revise: {
      title: 'Revise your plan, or keep it',
      lede: 'This is how each minister reacted. You can change your plan once to win them over. After this it is locked for the final vote.',
    },
    vote: {
      title: 'Hold the final vote',
      lede: 'Your plan is locked. Each minister now casts a final vote on it.',
    },
    decide: turn.canApprove
      ? { title: 'Your decision: approve or postpone',
          lede: `${turn.support} of 3 ministers support your plan. Approve it to commit ${plan} credits, or postpone to commit nothing.` }
      : { title: 'Not enough support to approve',
          lede: `Only ${turn.support} of 3 ministers support your plan, and ${VOTES_NEEDED} are needed. You can’t approve it. Postpone it and nothing is spent: you can try again next quarter.` },
    end: turn.keeping
      ? { title: 'End the quarter', lede: 'You kept the current programme. End the quarter to let work carry on.' }
      : { title: snapshot.outcome === 'ratified' ? 'Plan approved. End the quarter'
          : snapshot.outcome === 'deferred' ? 'Plan postponed. End the quarter' : 'End the quarter',
          lede: snapshot.outcome === 'deferred'
            ? 'Nothing was committed, so your budget is untouched. Ending the quarter moves time on.'
            : STEP_HELP.end },
    done: {
      title: 'Campaign complete',
      lede: `You played all ${OFFLINE_PILOT.total_quarters} quarters. See Progress for what was decided and delivered.`,
    },
  }
  const { title, lede } = copy[turn.step] ?? copy.done!
  return (
    <header className="view-head" aria-live="polite">
      <p className="kicker">
        {snapshot.closed ? 'Finished' : `Quarter ${q} of ${OFFLINE_PILOT.total_quarters} · ${QUARTERS[q - 1]?.label ?? ''}`}
        {!snapshot.closed && turn.step !== 'done' && !turn.keeping && ` · Step ${turn.index + 1} of ${STEPS.length}`}
      </p>
      <h1>{title}</h1>
      <p>{lede}</p>
    </header>
  )
}

function Audit({ snapshot }: { snapshot: Snapshot }) {
  const { send, saving } = useCampaign()
  const [family, setFamily] = useState<AuditFamily>('repairs')
  const [quarter, setQuarter] = useState(1)
  const [understood, setUnderstood] = useState(false)
  const reports = snapshot.delivery_history
  const shown = Math.min(quarter, reports.length)
  return (
    <section className="panel audit" aria-labelledby="audit-title">
      <div className="panel-head">
        <h2 id="audit-title"><Icon name="audit" size={18} />Check the real numbers (audit)</h2>
        <span className="tag tag-calm">Optional · no vote needed</span>
      </div>
      <p className="panel-lede">Ministers report their own progress, and reports can be optimistic. An audit checks what
        was really delivered in one area, for one finished quarter. The first audit is free; later ones cost 2 credits.</p>
      <div className="audit-fields">
        <label className="field"><span>Area</span>
          <select className="select" value={family} onChange={e => setFamily(e.target.value as AuditFamily)}>
            <option value="staffing">Staffing</option><option value="repairs">Repairs</option>
            <option value="access">Access</option>
          </select></label>
        <label className="field"><span>Finished quarter</span>
          <select className="select" value={shown} onChange={e => setQuarter(Number(e.target.value))}>
            {reports.map(r => <option key={r.quarter} value={r.quarter}>Q{r.quarter}</option>)}
          </select></label>
      </div>
      <label className="check"><input type="checkbox" checked={understood} onChange={e => setUnderstood(e.target.checked)} />
        <span>I understand the result isn’t shown on this screen yet. After stopping the game service, it can be read
          with <code>myth-report</code>.</span></label>
      <div className="room-buttons">
        <ActionButton disabled={!understood || saving}
          onClick={() => void send({ kind: 'order_audit', family, inspected_quarter: shown })}>Order the audit</ActionButton>
      </div>
    </section>
  )
}

function WhatIf() {
  const { createFork, forkId, saving } = useCampaign()
  return (
    <section className="panel whatif" aria-labelledby="whatif-title">
      <div className="panel-head">
        <h2 id="whatif-title">What if you’d chosen differently?</h2>
        <span className="tag tag-calm">Optional</span>
      </div>
      <p className="panel-lede">Branch the game at the decision you just made, then play the branch to compare. Your
        current game isn’t changed.</p>
      <div className="room-buttons">
        <ActionButton disabled={saving} onClick={() => void createFork()}>Create a what-if branch</ActionButton>
      </div>
      {forkId && <p role="status" className="alert alert-ok">
        <Icon name="check" size={16} />
        <span>Branch created. <a className="link" href={'?campaign=' + encodeURIComponent(forkId)}>Open the what-if branch</a>.
          Anything you do there leaves this game untouched.</span></p>}
    </section>
  )
}

export function PlayView({ allocation, onChange }: { allocation: Allocation; onChange: (next: Allocation) => void }) {
  const session = useCampaign()
  const { snapshot, send, saving, needsRetry, retry, error, dismissError, transport, integrityFailure } = session
  const turn = snapshot ? turnState(snapshot) : null
  const step = turn?.step
  const proposal = snapshot?.proposal ?? null
  const available = snapshot?.treasury.available_credits ?? 0
  const undelivered = snapshot ? undeliveredCredits(snapshot.treasury) : 0
  // What a new plan may commit: unspent credits plus the undelivered work it would replace.
  const fundable = snapshot ? fundableCredits(snapshot.treasury) : 0
  const programme = snapshot ? currentProgramme(snapshot.treasury) : NONE

  // Revising starts from the plan on the table, not from whatever the sliders last held.
  useEffect(() => {
    if (step === 'revise' && proposal) onChange(proposal)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, snapshot?.quarter, proposal?.staff, proposal?.repairs, proposal?.access])

  // A re-plan starts from the programme already promised (change what you want to change);
  // the first plan of the campaign starts from the first ready-made plan that fits.
  useEffect(() => {
    if (step !== 'plan' || !snapshot) return
    if (snapshot.quarter > 1 && undelivered > 0) { onChange(programme); return }
    if (committed(allocation) <= fundable) return
    onChange(POLICY_CARDS.find(c => committed(c.allocation) <= fundable)?.allocation ?? NONE)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, snapshot?.quarter, fundable])

  if (!snapshot || !turn) return <Gate />

  const q = snapshot.quarter
  const block = integrityFailure
  const idle = !saving && !block
  const fits = draftStatus(allocation) === 'valid' && committed(allocation) <= fundable
  const unchanged = proposal !== null && sameAllocation(allocation, proposal)
  const sameAsProgramme = undelivered > 0 && sameAllocation(allocation, programme)
  const nextQuarter = QUARTERS[q]?.label
  const go = (label: string, run: () => void, tone: 'primary' | 'normal' = 'primary', enabled = true): ReactNode => (
    <ActionButton tone={tone} disabled={!idle || !enabled} onClick={run}>
      {saving && tone === 'primary' ? 'Saving…' : label}
    </ActionButton>
  )
  const advance = () => {
    const kind = advanceKind(turn.step)
    // Wired only to steps that have a carry-on command; a mistake must never be a silent no-op.
    if (!kind) throw new Error(`No carry-on command for step "${turn.step}"`)
    void send({ kind })
  }

  const tooMuch = `This plan commits ${committed(allocation)} credits but you can plan with at most ${fundable}. Lower it first.`
  const whyNot = turn.step === 'plan' && fundable > 0 && !fits
    ? (committed(allocation) === 0 ? 'Choose a plan or move a slider first: an empty plan can’t be put to a vote.' : tooMuch)
    : turn.step === 'plan' && fundable > 0 && sameAsProgramme
      ? 'This is your current programme. Use “Keep the current programme” to carry on without a vote, or change something to propose a new plan.'
      : turn.step === 'revise' && !fits ? tooMuch
      : null

  let actions: ReactNode = null
  let body: ReactNode = null
  switch (turn.step) {
    case 'plan':
      body = fundable > 0
        ? <PlanBuilder allocation={allocation} budget={fundable} quarter={q} replacing={q > 1 ? undelivered : 0} onChange={onChange} />
        : <p className="empty-note"><Icon name="token" size={18} />Everything you funded has been delivered and nothing is left to spend.
            Carry on to the end of the campaign.</p>
      actions = <>
        {fundable > 0 && go('Propose this plan to the cabinet', () => void send({ kind: 'propose', allocation }), 'primary', fits && !sameAsProgramme)}
        {turn.canKeepProgramme && go('Keep the current programme (no vote)', () => void send({ kind: 'continue_quarter' }),
          fundable > 0 ? 'normal' : 'primary')}
      </>
      break
    case 'reactions':
      body = proposal && <PlanSummary plan={proposal} label="Your plan" />
      actions = go('Hear the ministers’ reactions', advance)
      break
    case 'revise':
      body = <PlanBuilder allocation={allocation} budget={fundable} quarter={q} replacing={q > 1 ? undelivered : 0} onChange={onChange} />
      actions = <>
        {go('Submit revised plan', () => void send({ kind: 'amend', allocation }, { then: { kind: 'advance_slice' } }),
          'primary', fits && !unchanged)}
        {go('Keep my plan as it is', advance, 'normal')}
      </>
      break
    case 'vote':
      body = proposal && <PlanSummary plan={proposal} label="Locked plan" />
      actions = go('Hold the final vote', advance)
      break
    case 'decide':
      body = proposal && <PlanSummary plan={proposal} label="Plan to decide" />
      {
        const approve = go(`Approve and commit ${proposal ? committed(proposal) : 0} credits`,
          () => void send({ kind: 'decide', ratify: true }), turn.canApprove ? 'primary' : 'normal', turn.canApprove)
        const postpone = go('Postpone (commit nothing)', () => void send({ kind: 'decide', ratify: false }),
          turn.canApprove ? 'normal' : 'primary')
        // When approving is impossible, the one viable move leads.
        actions = turn.canApprove ? <>{approve}{postpone}</> : <>{postpone}{approve}</>
      }
      break
    case 'end':
      body = <>
        {!turn.keeping && proposal && snapshot.outcome === 'ratified' && <PlanSummary plan={proposal} label="Approved plan" />}
        <p className="room-hint">{turn.keeping ? STEP_HELP.end
          : snapshot.outcome === 'ratified' ? 'The approved work is now promised. It will start arriving after the delays shown when you made the plan.'
          : 'Time moves on: ministers keep reporting, and organisations react.'}</p>
      </>
      actions = go(nextQuarter ? `End quarter ${q} and start quarter ${q + 1}` : `End the final quarter (Q${q})`,
        turn.keeping ? () => void send({ kind: 'finish_continuation' }) : advance)
      break
    default:
      body = <div className="finish">
        <p className="room-hint">{snapshot.decisions.length} decision{snapshot.decisions.length === 1 ? '' : 's'} recorded · {snapshot.treasury.service_spent} credits
          of work delivered · {snapshot.treasury.audit_spent} spent on audits.</p>
        {snapshot.decisions.length > 0 && <ol className="finish-list">
          {snapshot.decisions.map((d, i) => <li key={i}><strong>Q{d.quarter}</strong> {outcomeLabel(d.outcome)}
            {d.allocation && ` · ${committed(d.allocation)} credits`}</li>)}
        </ol>}
      </div>
  }

  // While you are responding to the ministers, their reactions matter more than the budget.
  const cabinetFirst = turn.step === 'revise' || turn.step === 'vote' || turn.step === 'decide'
  const latest = snapshot.delivery_history.at(-1)
  const auditOpen = !snapshot.closed && snapshot.phase === 'QuarterOpen' && !snapshot.continuation_pending &&
    q >= 3 && snapshot.delivery_history.length > 0 && idle
  const canFork = snapshot.phase === 'Resolved' && !snapshot.closed && idle

  return (
    <>
      <Headline snapshot={snapshot} turn={turn} />
      <StepTracker turn={turn} />

      {transport === 'reconnecting' && <p role="status" className="alert alert-warn">
        <Icon name="hourglass" size={16} /><span>Reconnecting to the game service… your progress is saved.</span></p>}
      {error && <div role="alert" className="alert alert-danger">
        <Icon name="alert" size={16} />
        <span>{error}</span>
        <span className="alert-actions">
          {needsRetry && <ActionButton size="sm" onClick={retry}>Try again</ActionButton>}
          <button type="button" className="icon-btn alert-x" onClick={dismissError} aria-label="Dismiss this message"><Icon name="close" size={16} /></button>
        </span>
      </div>}

      <div className="play-grid">
        <section className="panel panel-live step-card" aria-labelledby="step-title">
          <h2 id="step-title" className="sr-only">Your move</h2>
          {body}
          <div className="step-actions">{actions}</div>
          {whyNot && <p className="cta-why"><Icon name="lock" size={13} /> {whyNot}</p>}
          {turn.step === 'revise' && unchanged && fits && <p className="cta-why">Change the plan to submit a revision, or keep it as it is.</p>}
          {turn.step === 'decide' && !turn.canApprove && <p className="cta-why"><Icon name="lock" size={13} /> Approving needs {VOTES_NEEDED} of 3 supporting votes.</p>}
        </section>

        <aside className={`play-side${cabinetFirst ? ' cabinet-first' : ''}`} aria-label="Budget and cabinet">
          <section className="panel side-card side-budget" aria-labelledby="budget-title">
            <h2 id="budget-title" className="section-label">Budget</h2>
            <BudgetFigures treasury={snapshot.treasury} compact />
            <p className="hint">{OFFLINE_PILOT.credits} credits for the whole campaign; they never refill.
              {undelivered > 0 && ' A new plan replaces work that hasn’t been delivered yet; delivered work is spent for good.'}</p>
          </section>
          <section className="panel side-card side-cabinet" aria-labelledby="cabinet-title">
            <h2 id="cabinet-title" className="section-label">Your cabinet</h2>
            <CabinetStrip snapshot={snapshot} />
          </section>
          {latest && (
            <section className="panel side-card" aria-labelledby="report-title">
              <h2 id="report-title" className="section-label">Latest report · Q{latest.quarter}</h2>
              <ul className="report-lines">
                {FAMILIES.map((f, i) => (
                  <li key={f.id} className={`stat stat-${f.id}`}>
                    <Icon name={f.id} size={14} />{f.label}: {latest.reported_cumulative[i] ?? 0} of {latest.due_cumulative[i] ?? 0} due
                  </li>
                ))}
              </ul>
              <p className="hint">Reported by the ministers, not checked. Reports can be optimistic.</p>
            </section>
          )}
        </aside>
      </div>

      {auditOpen && <Audit snapshot={snapshot} />}
      {canFork && <WhatIf />}
      <TechnicalDetails />
    </>
  )
}
