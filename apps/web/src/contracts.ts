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
  /** Across both terms. */
  total_quarters: 12
  terms: 2
  quarters_per_term: 6
  /** Granted at the start of each term and added to anything unspent. */
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
  total_quarters: 12,
  terms: 2,
  quarters_per_term: 6,
  credits: 100,
  status: 'offline_preview',
}

export const EVIDENCE_LINES: readonly EvidenceLine[] = [
  { id: 'school-workforce', label: 'School workforce', category: 'public_source',
    detail: 'England school workforce reporting year 2025. Named as a source; not yet loaded into the game.' },
  { id: 'pupil-characteristics', label: 'Pupil characteristics', category: 'public_source',
    detail: 'DfE 2025/26 school-pupil release. Named as a source; not yet loaded into the game.' },
  { id: 'rural-income', label: 'Pupils by area and income', category: 'unverified',
    detail: 'No verified breakdown of pupils by area and income exists yet, so none is invented.' },
  { id: 'support-response', label: 'How the public reacts', category: 'authored_parameter',
    detail: 'How the public reacts to delivered work is a made-up game rule, not a forecast of English public opinion.' },
]
