/**
 * P1b display-only contracts. Authoritative Serde/ts-rs output replaces these
 * at P1-03; never send these shape definitions as unvalidated commands.
 */
export type Seat = 'Finance' | 'Education' | 'Infrastructure'
export type EvidenceKind = 'public_source' | 'authored_parameter' | 'unverified'
export type Availability = 'not_connected' | 'offline_preview' | 'live'

export interface PilotOverview {
  schema_version: 1
  country_iso: 'GB'
  jurisdiction: 'England'
  case_title: string
  locale: 'en-GB'
  start: '2027 Q1'
  total_quarters: 6
  credits: 100
  status: Availability
}

export interface EvidenceLine {
  id: string
  label: string
  category: EvidenceKind
  detail: string
}

export const OFFLINE_PILOT: PilotOverview = {
  schema_version: 1,
  country_iso: 'GB',
  jurisdiction: 'England',
  case_title: 'Education Delivery Pilot',
  locale: 'en-GB',
  start: '2027 Q1',
  total_quarters: 6,
  credits: 100,
  status: 'offline_preview',
}

export const EVIDENCE_LINES: readonly EvidenceLine[] = [
  { id: 'school-workforce', label: 'School workforce', category: 'public_source',
    detail: 'England school workforce reporting year 2025; source ingestion pending P2.' },
  { id: 'pupil-characteristics', label: 'Pupil characteristics', category: 'public_source',
    detail: 'DfE 2025/26 school-pupil release; source ingestion pending P2.' },
  { id: 'rural-income', label: 'Rural × income joint distribution', category: 'unverified',
    detail: 'No verified six-cell pupil joint distribution; do not invent a baseline.' },
  { id: 'support-response', label: 'Policy-response coefficients', category: 'authored_parameter',
    detail: 'Fictional service-response mechanics, not a forecast of English public opinion.' },
]
