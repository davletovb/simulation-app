export function whoItHits(effectPpm: readonly number[], groupIds: readonly string[], labels?: Readonly<Record<string, string>>): string
export function isTradeOff(effectPpm: readonly number[]): boolean
export function whoGainsAndLoses(effectPpm: readonly number[], groupIds: readonly string[], labels?: Readonly<Record<string, string>>): { helps: string; hurts: string }
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
export function inspectionText(
  scrutiny: { families: readonly string[]; owed_units: readonly number[] },
  options?: { landed?: boolean },
): string
export function windfallText(credits: number, options?: { landed?: boolean }): string
export function dayText(iso: string): string
export function realSourceText(real: { publisher: string; published: string; rule_label: string }): string
export function realNoteText(note: { rule_label: string | null; quarter: number | null }): string
export function reliefText(headline: { relief_cost: number | null; relief_cut_pct: number }): string
export function justIn<T extends { quarter: number }>(recent: readonly T[], currentQuarter: number): T[]
