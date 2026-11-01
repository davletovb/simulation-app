export type StepId = 'plan' | 'reactions' | 'revise' | 'vote' | 'decide' | 'end'
export interface StepInfo { readonly id: StepId; readonly label: string }
export const STEPS: readonly StepInfo[]
export const VOTES_NEEDED: number
export const MINISTERS: number

interface VoteLike { ministry: string; stage: string; position: string }
export function finalVoteCount(votes: readonly VoteLike[]): number
export function finalSupportCount(votes: readonly VoteLike[]): number

export interface TurnState {
  /** `night`: a term was won and the next has not begun. `done`: the game is over. */
  step: StepId | 'night' | 'done'
  /** Position in STEPS; STEPS.length on election night and once the game is over. */
  index: number
  /** "Keep the current programme" was chosen; only ending the quarter remains. */
  keeping: boolean
  /** Plan step only: the player may skip the vote and carry on (Q2 onwards). */
  canKeepProgramme: boolean
  /** Decide step only: enough ministers support the plan to approve it. */
  canApprove: boolean
  support: number
  votesIn: number
}
export function turnState(snapshot: {
  quarter: number
  phase: string
  closed: boolean
  continuation_pending: boolean
  votes: readonly VoteLike[]
}): TurnState
export function advanceKind(step: StepId | 'night' | 'done'): 'advance_slice' | null

interface TreasuryLike {
  available_credits: number
  encumbered_staff: number
  encumbered_repairs: number
  encumbered_access: number
}
/** Credits promised to approved plans but not yet delivered; a new plan may re-use them. */
export function undeliveredCredits(treasury: TreasuryLike): number
/** The most a new plan may commit: unspent credits plus the undelivered work it would replace. */
export function fundableCredits(treasury: TreasuryLike): number
/** The current programme (still-promised work) expressed as a plan. */
export function currentProgramme(treasury: TreasuryLike): { staff: number; repairs: number; access: number }
