export function whoItHits(effectPpm: readonly number[], groupIds: readonly string[]): string
export function overallText(overallPpm: number, options?: { approximate?: boolean }): string
export function landsText(headlineQuarter: number, currentQuarter: number): string
export function canRespond(
  headline: { relief_cost: number | null; paid: number | null; tone: string },
  state: { available: number; step: string; busy: boolean },
): { ok: boolean; why: string | null }
export function disruptionText(
  disruption: { family: string; keep_pct: number; quarters: number },
  options?: { delayedUnits?: number },
): string
export function reliefText(headline: { relief_cost: number | null; relief_cut_pct: number }): string
export function justIn<T extends { quarter: number }>(recent: readonly T[], currentQuarter: number): T[]
