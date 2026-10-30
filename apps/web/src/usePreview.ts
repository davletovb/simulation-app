import { useCallback, useEffect, useState } from 'react'
import { FAMILIES, QUARTERS, draftStatus, type Allocation, type FamilyForecast } from './game'

type ServiceFamily = 'staffing' | 'repairs' | 'access'
interface ServiceLine { family: ServiceFamily; per_quarter: number[]; outstanding: number }
interface ServicePreview {
  kind: 'optimistic_upper_bound'
  calculator: 'myth-domain'
  authoritative_simulation: false
  conditional_access_bonus: boolean
  families: ServiceLine[]
}
type PreviewState = { key: string; rows: FamilyForecast[] | null; status: 'ok' | 'unavailable' }
const familyKey: Record<ServiceFamily, string> = {
  staffing: 'staff', repairs: 'repairs', access: 'access',
}

function parsePreview(raw: unknown): FamilyForecast[] {
  if (typeof raw !== 'object' || raw === null) throw new Error('Bad server preview')
  const response = raw as Partial<ServicePreview>
  if (response.kind !== 'optimistic_upper_bound' || response.calculator !== 'myth-domain' ||
      response.authoritative_simulation !== false || !Array.isArray(response.families) ||
      response.families.length !== FAMILIES.length) throw new Error('Unexpected preview contract')
  const source = new Map<string, ServiceLine>()
  for (const item of response.families) {
    if (!item || !(item.family in familyKey) || source.has(item.family)) {
      throw new Error('Unknown or duplicate family')
    }
    source.set(familyKey[item.family], item)
  }
  return FAMILIES.map(family => {
    const row = source.get(family.id)
    if (!row || !Array.isArray(row.per_quarter) || row.per_quarter.length !== QUARTERS.length ||
      row.per_quarter.some(n => !Number.isInteger(n) || n < 0 || n > 100) ||
      !Number.isInteger(row.outstanding) || row.outstanding < 0 || row.outstanding > 100) {
      throw new Error('Invalid preview units')
    }
    return { family, perQuarter: row.per_quarter, outstanding: row.outstanding }
  })
}

/** Only Rust returns delivery forecasts. No duplicate TS calculation or fallback. */
export interface PreviewResult {
  rows: FamilyForecast[] | null
  status: 'invalid' | 'loading' | 'updating' | 'unavailable' | 'ready'
  retry: () => void
}
export function usePreview(allocation: Allocation): PreviewResult {
  const key = [allocation.staff, allocation.repairs, allocation.access].join(':')
  const valid = draftStatus(allocation) === 'valid'
  const [state, setState] = useState<PreviewState | null>(null)
  const [retryEpoch, setRetryEpoch] = useState(0)
  const retry = useCallback(() => {
    setState(null)
    setRetryEpoch(n => n + 1)
  }, [])
  useEffect(() => {
    if (!valid) return
    const abort = new AbortController()
    // Only send the final slider value after a short quiet window.
    const timeout = window.setTimeout(() => {
      void (async () => {
        try {
          const response = await fetch('/api/preview', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ allocation }),
            signal: abort.signal,
            cache: 'no-store',
          })
          if (!response.ok) throw new Error('Rust preview service unavailable')
          const rows = parsePreview(await response.json() as unknown)
          if (!abort.signal.aborted) setState({ key, rows, status: 'ok' })
        } catch {
          if (!abort.signal.aborted) setState({ key, rows: null, status: 'unavailable' })
        }
      })()
    }, 180)
    return () => {
      window.clearTimeout(timeout)
      abort.abort()
    }
  }, [key, valid, allocation.staff, allocation.repairs, allocation.access, retryEpoch])
  if (!valid) return { rows: null, status: 'invalid', retry }
  if (state?.key === key) return {
    rows: state.rows, status: state.status === 'ok' ? 'ready' : 'unavailable', retry,
  }
  if (state?.rows) return { rows: state.rows, status: 'updating', retry }
  return { rows: null, status: 'loading', retry }
}
