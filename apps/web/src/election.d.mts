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
  settled_group_ppm: readonly number[]
  weighted_support_ppm: number
  blocs_held_ppm: number
  min_support_ppm: number
  min_blocs_ppm: number
  held_above_ppm: number
}
export interface Bar { key: 'support' | 'blocs'; label: string; value: number; need: number; met: boolean }
export function bars(election: ElectionLike): [Bar, Bar]

export type VerdictKey = 'reelected' | 'narrow' | 'heavy' | 'on-course' | 'behind'
export interface Verdict { key: VerdictKey; label: string; final: boolean; won: boolean }
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
  }
}
export function groups(snapshot: SnapshotLike, when?: 'current' | 'settled'): GroupView[]
export function leftBehind(snapshot: SnapshotLike): GroupView[]
export interface Legacy {
  delivered: number
  unfinished: number
  unspent: number
  audits: number
  total: number
  share: number
  key: 'delivered' | 'partly' | 'short' | 'missed'
  label: string
}
export function legacy(snapshot: SnapshotLike): Legacy
