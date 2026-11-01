/**
 * Display-only game data and pure preview helpers for the offline shell.
 * Values mirror docs/p0-case-contract.md (P0-02/P0-03) so the interface can
 * explain rules and fictional faction interests. Delivery calculations now
 * come only from the local Rust service; the P3 reducer remains authoritative
 * for commitments, actual delivery and resulting society effects.
 */
import { OFFLINE_PILOT, type PilotOverview, type Seat } from './contracts'
import { VOTES_NEEDED, MINISTERS as MINISTER_COUNT } from './turn.mjs'

export { VOTES_NEEDED, MINISTER_COUNT }

const COUNTRY_NAMES: Record<PilotOverview['country_iso'], string> = { GB: 'United Kingdom' }
export const COUNTRY_NAME = COUNTRY_NAMES[OFFLINE_PILOT.country_iso]
/** P0-01 requires the selector and jurisdiction to be shown together. */
export const PLACE_LABEL = `${COUNTRY_NAME} · ${OFFLINE_PILOT.jurisdiction} education pilot`

export type Family = 'staff' | 'repairs' | 'access'
export type Allocation = Record<Family, number>
export type PackageId = 'balanced' | 'staff_first' | 'access_first'
export type DraftStatus = 'empty' | 'valid' | 'over_budget'

export interface FamilyInfo {
  id: Family
  label: string
  /** What this spending buys, in everyday words. */
  blurb: string
  unit: string
  /** Nominal maximum delivered units per quarter. */
  quarterCap: number
  /** Quarters after ratification before the first eligible delivery. */
  lag: number
}

export const FAMILIES: readonly FamilyInfo[] = [
  { id: 'staff', label: 'Staffing', blurb: 'Teachers and support staff in schools', unit: 'staff-quarter slot', quarterCap: 12, lag: 1 },
  { id: 'repairs', label: 'Repairs', blurb: 'Fixing school buildings', unit: 'repair-work unit', quarterCap: 10, lag: 2 },
  { id: 'access', label: 'Access', blurb: 'Local support that helps families use school services', unit: 'access-support unit', quarterCap: 4, lag: 0 },
]

export interface PolicyCard {
  id: PackageId
  name: string
  tagline: string
  allocation: Allocation
}

/**
 * Ready-made plans. Only plans a player can actually submit appear here: an
 * empty plan is rejected, and a plan above the budget is never ratifiable.
 * (To spend nothing, postpone after the vote, or keep the programme from Q2.)
 */
export const POLICY_CARDS: readonly PolicyCard[] = [
  { id: 'balanced', name: 'Balanced', tagline: 'Staffing, with essential repairs',
    allocation: { staff: 50, repairs: 40, access: 10 } },
  { id: 'staff_first', name: 'Staffing first', tagline: 'More staff, fewer repairs',
    allocation: { staff: 60, repairs: 30, access: 10 } },
  { id: 'access_first', name: 'Access first', tagline: 'Wider reach; keeps 10 credits in reserve',
    allocation: { staff: 40, repairs: 30, access: 20 } },
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

/**
 * When work funded by a plan approved in `quarter` (1-based) can first arrive,
 * or null if that is after the campaign ends. Delivery starts `lag` quarters
 * after approval (P0 scheduling rule), not at a fixed calendar quarter.
 */
export function firstDeliveryLabel(family: FamilyInfo, quarter: number): string | null {
  return QUARTERS[quarter - 1 + family.lag]?.label ?? null
}

/** First quarter (1-based) in which the campaign's first audit is free. */
export const FIRST_FREE_AUDIT_QUARTER = 3
export const PAID_AUDIT_COST = 2

export interface FamilyForecast {
  family: FamilyInfo
  perQuarter: readonly number[]
  outstanding: number
}

/** Delivery values are calculated only by the Rust offline service. */
export interface MinisterInfo {
  name: Seat
  /** Wire id used by the service (`finance`, `education`, `infrastructure`). */
  id: string
  role: string
  /** What this minister does, in everyday words. */
  duty: string
  /** Delivery family whose actual effort this minister sets privately. */
  owns?: Family
}

export const MINISTERS: readonly MinisterInfo[] = [
  { name: 'Finance', id: 'finance', role: 'Finance minister',
    duty: 'Checks the plan is affordable and that any conditions can be enforced.' },
  { name: 'Education', id: 'education', role: 'Education minister',
    duty: 'Delivers Staffing. Decides, privately, how hard to push it.', owns: 'staff' },
  { name: 'Infrastructure', id: 'infrastructure', role: 'Infrastructure minister',
    duty: 'Delivers Repairs. Decides, privately, how hard to push it.', owns: 'repairs' },
]

/** The six steps of a quarter, explained once and reused by the briefing, tracker and cabinet page. */
export const STEP_HELP: Readonly<Record<'plan' | 'reactions' | 'revise' | 'vote' | 'decide' | 'end', string>> = {
  plan: 'Split your budget between Staffing, Repairs and Access. From the second quarter you can instead keep the current programme and skip the vote.',
  reactions: 'Each minister says whether they support your plan, support it with conditions, or oppose it.',
  revise: `You may change the plan once, in response. Or keep it as it is.`,
  vote: 'The plan is now locked. Each minister casts a final vote.',
  decide: `If at least ${VOTES_NEEDED} of 3 ministers support it, you can approve it and commit the budget. Or postpone and spend nothing.`,
  end: 'Time moves on: work is delivered (late and limited), organisations react, and the next quarter begins.',
}

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
