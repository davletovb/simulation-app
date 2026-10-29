/**
 * Display-only game data and pure preview helpers for the offline shell.
 * Values mirror docs/p0-case-contract.md (P0-02/P0-03) so the interface can
 * explain the rules; the P3 reducer and the server-side preview stay
 * authoritative and replace `bestCaseSchedule` before anything is committed.
 */
import { OFFLINE_PILOT, type PilotOverview, type Seat } from './contracts'

const COUNTRY_NAMES: Record<PilotOverview['country_iso'], string> = { GB: 'United Kingdom' }
export const COUNTRY_NAME = COUNTRY_NAMES[OFFLINE_PILOT.country_iso]
/** P0-01 requires the selector and jurisdiction to be shown together. */
export const PLACE_LABEL = `${COUNTRY_NAME} · ${OFFLINE_PILOT.jurisdiction} education pilot`

export type Family = 'staff' | 'repairs' | 'access'
export type Allocation = Record<Family, number>
export type PackageId = 'balanced' | 'staff_first' | 'access_first' | 'oversubscribed' | 'defer'
export type DraftStatus = 'empty' | 'valid' | 'over_budget'

export interface FamilyInfo {
  id: Family
  label: string
  unit: string
  /** Nominal maximum delivered units per quarter. */
  quarterCap: number
  /** Quarters after ratification before the first eligible delivery. */
  lag: number
}

export const FAMILIES: readonly FamilyInfo[] = [
  { id: 'staff', label: 'Staffing', unit: 'staff-quarter slot', quarterCap: 12, lag: 1 },
  { id: 'repairs', label: 'Repairs', unit: 'repair-work unit', quarterCap: 10, lag: 2 },
  { id: 'access', label: 'Access', unit: 'access-support unit', quarterCap: 4, lag: 0 },
]

export interface PolicyCard {
  id: PackageId
  name: string
  tagline: string
  allocation: Allocation
}

export const POLICY_CARDS: readonly PolicyCard[] = [
  { id: 'balanced', name: 'Balanced delivery', tagline: 'Staffing with essential repairs',
    allocation: { staff: 50, repairs: 40, access: 10 } },
  { id: 'staff_first', name: 'Staffing first', tagline: 'More staff, fewer repairs',
    allocation: { staff: 60, repairs: 30, access: 10 } },
  { id: 'access_first', name: 'Access first', tagline: 'Wider reach, 10 credits held back',
    allocation: { staff: 40, repairs: 30, access: 20 } },
  { id: 'oversubscribed', name: 'Everything at once', tagline: 'Spends more than the pilot holds',
    allocation: { staff: 60, repairs: 40, access: 10 } },
  { id: 'defer', name: 'Defer', tagline: 'Commit nothing this quarter',
    allocation: { staff: 0, repairs: 0, access: 0 } },
]

export const DEFAULT_CARD: PolicyCard = POLICY_CARDS[0]!

export const committed = (a: Allocation): number => a.staff + a.repairs + a.access
export const available = (a: Allocation): number => OFFLINE_PILOT.credits - committed(a)

export function draftStatus(a: Allocation): DraftStatus {
  const total = committed(a)
  if (total === 0) return 'empty'
  return total > OFFLINE_PILOT.credits ? 'over_budget' : 'valid'
}

export const sameAllocation = (a: Allocation, b: Allocation): boolean =>
  a.staff === b.staff && a.repairs === b.repairs && a.access === b.access

const [startYear = 2027, startQuarter = 1] = OFFLINE_PILOT.start.split(' Q').map(Number)

/** Quarter labels for the campaign, e.g. `Q1 2027` … `Q2 2028`. */
export const QUARTERS: readonly { short: string; year: number; label: string }[] =
  Array.from({ length: OFFLINE_PILOT.total_quarters }, (_, i) => {
    const offset = startQuarter - 1 + i
    const year = startYear + Math.floor(offset / 4)
    const short = `Q${(offset % 4) + 1}`
    return { short, year, label: `${short} ${year}` }
  })

export const lastQuarterLabel = (): string => QUARTERS[QUARTERS.length - 1]?.label ?? ''

/** First quarter (1-based) in which the campaign's first audit is free. */
export const FIRST_FREE_AUDIT_QUARTER = 3
export const PAID_AUDIT_COST = 2

export interface FamilyForecast {
  family: FamilyInfo
  perQuarter: readonly number[]
  outstanding: number
}

/**
 * Optimistic upper bound for one scope ratified before Q1, with full effort,
 * no blocked tranches, slowdown or shock. In the access family, include the
 * *attainable*, never guaranteed Community Access Network +1 next-quarter
 * bonus if access funding >=10 and eligible funded access remains. The first
 * bonus cannot operate until Q2 because Q1's faction action affects Q2.
 * Access resources are sufficient for each Q2..Q6 bonus: initial 4 volunteer
 * days plus the authored replenishment per quarter.
 *
 * This is a display-only best-case ceiling, not a deterministic prediction or
 * a substitute for the authoritative future P3 society/treasury reducer.
 * Temporary by decision: the Rust engine owns all authoritative previews, so
 * replace this with its preview endpoint once one exists.
 */
export function bestCaseSchedule(a: Allocation): FamilyForecast[] {
  return FAMILIES.map(family => {
    let remaining = a[family.id]
    const perQuarter = QUARTERS.map((_, i) => {
      if (i < family.lag) return 0
      const attainableAccessBonus =
        family.id === 'access' && i > 0 && a.access >= 10 && remaining > 0 ? 1 : 0
      const familyCapacity = family.quarterCap + attainableAccessBonus
      const delivered = Math.min(remaining, familyCapacity)
      remaining -= delivered
      return delivered
    })
    return { family, perQuarter, outstanding: remaining }
  })
}

export interface SeatInfo {
  name: Seat
  title: string
  mandate: string
  powers: readonly string[]
  /** Delivery family whose actual effort this seat sets privately. */
  owns?: Family
}

export const SEATS: readonly SeatInfo[] = [
  { name: 'Finance', title: 'Treasury seat',
    mandate: 'Checks that every package is affordable and that its milestones can be enforced.',
    powers: ['Votes', 'Funding conditions'] },
  { name: 'Education', title: 'Education seat',
    mandate: 'Proposes staffing, and alone decides how hard staffing is delivered.',
    powers: ['Votes', 'Proposes staffing'], owns: 'staff' },
  { name: 'Infrastructure', title: 'Facilities seat',
    mandate: 'Proposes repairs and tranches, and alone decides how hard repairs are delivered.',
    powers: ['Votes', 'Proposes repairs'], owns: 'repairs' },
]

export const VOTES_NEEDED = 2

export const MEETING_PHASES: readonly { label: string; detail: string }[] = [
  { label: 'Freeze', detail: 'Your package and each seat’s view are locked.' },
  { label: 'First positions', detail: 'All three seats answer at once: support, conditional or oppose.' },
  { label: 'Your amendment', detail: 'Change the package once, or keep it.' },
  { label: 'Final votes', detail: 'Seats vote on the frozen final version.' },
  { label: 'Ratify', detail: `${VOTES_NEEDED} of 3 support and the checks pass: you decide.` },
]

export interface Pressure { kind: 'risk' | 'boost'; text: string }

/**
 * Layered visibility: `interest` and pressure text are the organisation's public
 * stance; `rule` is the exact authored mechanic, shown only behind "View mechanics".
 * Thresholds stay in `pressure`, so no public copy repeats them.
 */
export interface Faction {
  id: string
  name: string
  interest: string
  resource: string
  start: number
  max: number
  perQuarter: number
  rule: string
  pressure: (a: Allocation) => Pressure | null
}

export const FACTIONS: readonly Faction[] = [
  { id: 'ssa', name: 'School Staff Association', resource: 'Influence', start: 10, max: 20, perQuarter: 2,
    interest: 'Wants sustained investment in school staffing, and has organised slowdowns when it judged staffing neglected.',
    rule: 'If funded staffing is below 50, it can spend 4 influence to cap next quarter’s staffing effort at 80%.',
    pressure: a => a.staff > 0 && a.staff < 50
      ? { kind: 'risk', text: 'Concerned this staffing investment is too low, and may organise a slowdown.' }
      : null },
  { id: 'can', name: 'Community Access Network', resource: 'Volunteer days', start: 4, max: 12, perQuarter: 1,
    interest: 'Wants local access support funded, and offers volunteer help where it sees a real commitment.',
    rule: 'If access funding is 10 or more, it can spend a volunteer day to add 1 access capacity next quarter.',
    pressure: a => a.access >= 10
      ? { kind: 'boost', text: 'Sees a real access commitment, and may volunteer to speed up access delivery.' }
      : null },
]

export const COHORT_INCOMES = ['Low', 'Middle', 'High'] as const
export const COHORT_AREAS = ['Urban', 'Rural'] as const
export const COHORT_CELLS = COHORT_INCOMES.length * COHORT_AREAS.length
/** Authored starting support for every cohort cell; not a poll. */
export const COHORT_START_SUPPORT = 50
