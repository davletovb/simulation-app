export const MATERIAL_PPM: number
export function shortGroup(id: string, labels?: Readonly<Record<string, string>>): string
export function planEffects(allocation: { staff: number; repairs: number; access: number }, net: readonly (readonly number[])[]): number[]
export function interestsText(effectsPpm: readonly number[], groupIds: readonly string[], labels?: Readonly<Record<string, string>>): string
