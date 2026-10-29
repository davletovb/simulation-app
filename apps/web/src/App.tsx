import { useState } from 'react'
import { EVIDENCE_LINES, OFFLINE_PILOT, type Seat } from './contracts'

type Tab = 'briefing' | 'cabinet' | 'outcomes' | 'evidence'
type PackageId = 'balanced' | 'staff_first' | 'access_first' | 'defer'
type Budget = { staff: number; repairs: number; access: number }
type Candidate = { id: PackageId; label: string; subline: string; allocation: Budget }

const NAV: readonly { id: Tab; label: string; index: string }[] = [
  { id: 'briefing', label: 'Briefing', index: '01' },
  { id: 'cabinet', label: 'The cabinet', index: '02' },
  { id: 'outcomes', label: 'Outcomes', index: '03' },
  { id: 'evidence', label: 'Evidence', index: '04' },
]

const CANDIDATES: readonly Candidate[] = [
  { id: 'balanced', label: 'Balanced delivery', subline: 'Staffing + essential repairs',
    allocation: { staff: 50, repairs: 40, access: 10 } },
  { id: 'staff_first', label: 'Staffing first', subline: 'More teachers, fewer repairs',
    allocation: { staff: 60, repairs: 30, access: 10 } },
  { id: 'access_first', label: 'Access first', subline: 'Service reach, held reserve',
    allocation: { staff: 40, repairs: 30, access: 20 } },
  { id: 'defer', label: 'Defer', subline: 'Make no funding commitment',
    allocation: { staff: 0, repairs: 0, access: 0 } },
]

const SEATS: readonly { name: Seat; role: string; focus: string }[] = [
  { name: 'Finance', role: 'Funding & milestones', focus: 'Are commitments affordable?' },
  { name: 'Education', role: 'Staff delivery', focus: 'Can staffing reach the classroom?' },
  { name: 'Infrastructure', role: 'Repairs & facilities', focus: 'Can repairs be delivered on time?' },
]

const asPercent = (n: number): string => `${n}%`

function BudgetBars({ allocation }: { allocation: Budget }) {
  const available = 100 - allocation.staff - allocation.repairs - allocation.access
  const segments = [
    { name: 'Staffing', value: allocation.staff, cls: 'bar-staff' },
    { name: 'Repairs', value: allocation.repairs, cls: 'bar-repairs' },
    { name: 'Access', value: allocation.access, cls: 'bar-access' },
    { name: 'Uncommitted', value: available, cls: 'bar-reserve' },
  ]
  return (
    <div className="budget">
      <div className="budget-track" role="img"
        aria-label={segments.map(s => `${s.name}: ${s.value} credits`).join(', ')}>
        {segments.filter(s => s.value > 0).map(s => (
          <div className={s.cls} key={s.name} style={{ width: asPercent(s.value) }} />
        ))}
      </div>
      <div className="budget-legend">
        {segments.map(s => (
          <div className="budget-line" key={s.name}>
            <span className="legend-label"><span className={`legend-dot ${s.cls}`} />{s.name}</span>
            <span className="mono">{s.value}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

function SectionTitle({ eyebrow, title, body }: {
  eyebrow: string; title: string; body: string
}) {
  return (
    <div className="section-title">
      <div className="eyebrow">{eyebrow}</div>
      <h2>{title}</h2>
      <p>{body}</p>
    </div>
  )
}

export function App() {
  const [tab, setTab] = useState<Tab>('briefing')
  const [selectedId, setSelectedId] = useState<PackageId>('balanced')
  const candidate = CANDIDATES.find(x => x.id === selectedId) ?? CANDIDATES[0]!
  const budgetLeft = 100 - Object.values(candidate.allocation).reduce((sum, n) => sum + n, 0)

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true"><span>✧</span></div>
          <div><div className="brand-name">MYTH MAKER</div><div className="brand-tag">The art of governing</div></div>
        </div>
        <div className="sidebar-divider" />
        <div className="side-label">YOUR CAMPAIGN</div>
        <div className="country-card">
          <div className="country-flag" aria-hidden="true">🇬🇧</div>
          <div><strong>United Kingdom</strong><small>England · education pilot</small></div>
          <span className="small-chevron">⌄</span>
        </div>
        <div className="side-label nav-label">WORKSPACE</div>
        <nav aria-label="Campaign navigation">
          {NAV.map(item => (
            <button key={item.id} type="button"
              className={`nav-link ${item.id === tab ? 'active' : ''}`}
              onClick={() => setTab(item.id)} aria-current={item.id === tab ? 'page' : undefined}>
              <span className="nav-number">{item.index}</span>{item.label}
              {item.id === tab && <span className="nav-active-mark" aria-hidden="true" />}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="connection"><span className="connection-dot" />Offline interface preview</div>
          <p>Not connected to a simulation service. No model calls or policy commits are made here.</p>
          <div className="sidebar-version">FRAMEWORK v0.8.1 · P1 FOUNDATION</div>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <span className="breadcrumb">SIMULATION / ENGLAND / {tab.toUpperCase()}</span>
          <div className="top-actions">
            <span className="header-status"><span className="tiny-dot" /> Local preview</span>
            <span className="quarter-pill">Q1 / Q6</span>
          </div>
        </header>

        <div className="content">
          <div className="hero">
            <div className="hero-detail">
              <span className="hero-kicker"><span className="accent-line" /> GOVERNANCE SIMULATION · 2027</span>
              <h1>Decisions have<br /><em>consequences.</em></h1>
              <p>Guide a fictional education programme through six quarters. Balance services, funding, and the interests of a cabinet whose commitments matter.</p>
              <div className="hero-pills"><span>United Kingdom · England</span><span>Six quarters</span><span>100 fictional credits</span></div>
            </div>
            <div className="hero-orbit" aria-hidden="true">
              <div className="orbit orbit-one" /><div className="orbit orbit-two" /><div className="orbit orbit-three" />
              <div className="orbit-center">MM</div>
              <div className="orbit-node node-a" /><div className="orbit-node node-b" /><div className="orbit-node node-c" />
            </div>
          </div>

          <div className="disclaimer" role="note">
            <strong>Fictional data twin</strong>
            <span>Illustrative mechanics, not government forecasts. England is the education jurisdiction; this is not the whole UK.</span>
          </div>

          {tab === 'briefing' && (
            <section className="workspace-section">
              <SectionTitle eyebrow="01 / THE CHALLENGE" title="Allocate your first mandate"
                body="Explore illustrative allocations before any negotiation. These figures are fictional programme credits, not pounds sterling or actual government spending." />
              <div className="two-column">
                <div className="panel choice-panel">
                  <div className="panel-top"><h3>Policy directions</h3><span className="muted-small">SELECT TO PREVIEW</span></div>
                  <div className="choices">
                    {CANDIDATES.map(option => (
                      <button className={`choice ${selectedId === option.id ? 'choice-selected' : ''}`}
                        type="button" key={option.id} onClick={() => setSelectedId(option.id)}
                        aria-pressed={selectedId === option.id}>
                        <span className="choice-radio" />
                        <span><strong>{option.label}</strong><small>{option.subline}</small></span>
                        <span className="choice-value mono">{option.allocation.staff + option.allocation.repairs + option.allocation.access}</span>
                      </button>
                    ))}
                  </div>
                </div>
                <div className="panel">
                  <div className="panel-top"><h3>Allocation preview</h3><span className="tag">NOT COMMITTED</span></div>
                  <div className="metric-row">
                    <div><span className="metric-label">Committed in draft</span>
                      <div className="metric"><strong>{100 - budgetLeft}</strong><span>/ 100</span></div></div>
                    <div><span className="metric-label">Available reserve</span>
                      <div className="metric"><strong>{budgetLeft}</strong><span>credits</span></div></div>
                  </div>
                  <BudgetBars allocation={candidate.allocation} />
                  <p className="panel-note">This is a local calculation only. Cabinet votes, funding conditions, delivery, and social responses are not yet simulated.</p>
                </div>
              </div>
            </section>
          )}

          {tab === 'cabinet' && (
            <section className="workspace-section">
              <SectionTitle eyebrow="02 / THE INSTITUTION" title="Three seats. Different priorities."
                body="The fictional Education Delivery Board will negotiate typed, funded proposals. Ministers' private effort and preferences are not public observations." />
              <div className="seat-grid">
                {SEATS.map((seat, idx) => (
                  <article key={seat.name} className="panel seat-panel">
                    <div className="seat-number">0{idx + 1}</div>
                    <div className="seat-monogram">{seat.name[0]}</div>
                    <h3>{seat.name}</h3>
                    <p className="seat-role">{seat.role}</p>
                    <div className="seat-focus">“{seat.focus}”</div>
                    <div className="seat-state"><span className="tiny-dot" /> Awaiting offline backend</div>
                  </article>
                ))}
              </div>
            </section>
          )}

          {tab === 'outcomes' && (
            <section className="workspace-section">
              <SectionTitle eyebrow="03 / ACCOUNTABILITY" title="Track what actually changes."
                body="Policy approval, delivery, audited truth, and social response will be tracked separately, with timing delays and explicit uncertainty." />
              <div className="panel empty-state">
                <div className="empty-symbol" aria-hidden="true">⌁</div>
                <h3>No outcomes yet</h3>
                <p>A simulation has not been started. The P3 reducer and P4 Hiyal replay adapter must produce these results before this view can show authentic delivery or cohort effects.</p>
              </div>
            </section>
          )}

          {tab === 'evidence' && (
            <section className="workspace-section">
              <SectionTitle eyebrow="04 / EVIDENCE & ASSUMPTIONS" title="Know what is observed."
                body="Each public-data input needs a reference date, source, definition, and archived checksum. Authored mechanics and missing evidence must remain distinguishable." />
              <div className="panel evidence-panel">
                {EVIDENCE_LINES.map(item => (
                  <article className="evidence-row" key={item.id}>
                    <div>
                      <h3>{item.label}</h3><p>{item.detail}</p>
                    </div>
                    <span className={`evidence-tag evidence-${item.category}`}>
                      {item.category === 'public_source' ? 'Source identified' :
                        item.category === 'authored_parameter' ? 'Authored rule' : 'Missing evidence'}
                    </span>
                  </article>
                ))}
                <div className="panel-note">Public source discovery is not the same as a validated P2 country pack. No figures are treated as ingested here.</div>
              </div>
            </section>
          )}
        </div>

        <footer className="footer">
          <span>MYTH MAKER — LOCAL FOUNDATION</span>
          <span>Schema {OFFLINE_PILOT.schema_version} · {OFFLINE_PILOT.locale} · No live inference</span>
        </footer>
      </main>
    </div>
  )
}
