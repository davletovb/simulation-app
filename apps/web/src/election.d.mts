export const GROUP_LABELS: Readonly<Record<string, string>>
export const NARROW_DEFEAT_PPM: number
/** Parts per million as a percentage, truncated (never rounded up) to one decimal. */
export function pct(ppm: number): number
export function pctText(ppm: number): string
export function pointsText(deltaPpm: number): string
/** Points below a bar, rounded up so a miss never reads as 0.0. */
export function shortByText(shortfallPpm: number): string

export interface ElectionLike {
  status: 'projected' | 'won' | 'lost'
  /** The term this election ends (1-based). There is no fixed number of terms: a win starts another. */
  term: number
  /** The longest tenure the engine can record: winning this election ends the game. */
  ceiling: boolean
  settled_group_ppm: readonly number[]
  weighted_support_ppm: number
  /** Voters in groups backing the player. Information only: it plays no part in who wins. */
  blocs_held_ppm: number
  min_support_ppm: number
  held_above_ppm: number
}
export interface Bar { key: 'support'; label: string; value: number; need: number; met: boolean }
/** The one bar an election is judged on. */
export function supportBar(election: ElectionLike): Bar

export interface Backing { groups: number; of: number; voters: number; neutral: boolean; text: string }
/** Which groups are behind the player, as information only. */
export function backing(election: ElectionLike): Backing

/** The election at the engine's limit: winning it ends the game instead of starting another term. */
export function isCeiling(election: ElectionLike): boolean

export type VerdictKey = 'reelected' | 'tenure' | 'narrow' | 'heavy' | 'on-course' | 'behind'
export interface Verdict { key: VerdictKey; label: string; final: boolean; won: boolean; ceiling: boolean }
export function verdict(election: ElectionLike): Verdict
export function headline(election: ElectionLike): { title: string; lede: string }

export interface GroupView {
  id: string
  label: string
  ppm: number
  /** The group's share of the voters, in ppm. */
  weight: number
  held: boolean
  /** Already queued to land next quarter, in ppm. */
  pending: number
}
interface SnapshotLike {
  support: {
    group_ids: readonly string[]
    group_ppm: readonly number[]
    pending_delta_ppm: readonly number[]
    weight_ppm: readonly number[]
  }
  election: ElectionLike
  treasury: {
    available_credits: number
    encumbered_staff: number
    encumbered_repairs: number
    encumbered_access: number
    service_spent: number
    audit_spent: number
    /** Credits spent answering headlines. */
    relief_spent?: number
  }
}
export function groups(snapshot: SnapshotLike, when?: 'current' | 'settled'): GroupView[]
export function leftBehind(snapshot: SnapshotLike): GroupView[]
export interface Legacy {
  delivered: number
  unfinished: number
  unspent: number
  audits: number
  /** Credits spent answering headlines: they buy votes, not delivered work. */
  relief: number
  total: number
  share: number
  key: 'delivered' | 'partly' | 'short' | 'missed'
  label: string
}
export function legacy(snapshot: SnapshotLike): Legacy
