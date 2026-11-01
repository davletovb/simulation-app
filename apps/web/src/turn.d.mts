export type StepId = 'plan' | 'reactions' | 'revise' | 'vote' | 'decide' | 'end'
export interface StepInfo { readonly id: StepId; readonly label: string }
export const STEPS: readonly StepInfo[]
export const VOTES_NEEDED: number
export const MINISTERS: number

interface VoteLike { ministry: string; stage: string; position: string }
export function finalVoteCount(votes: readonly VoteLike[]): number
export function finalSupportCount(votes: readonly VoteLike[]): number

export interface TurnState {
  step: StepId | 'done'
  /** Position in STEPS; STEPS.length once the campaign is complete. */
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
export function advanceKind(step: StepId | 'done'): 'advance_slice' | null
