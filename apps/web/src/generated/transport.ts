// GENERATED from myth-contracts. DO NOT EDIT.
// All *_version and expected_world_version fields are decimal strings.

export type Allocation = { staff: number, repairs: number, access: number, };

export type CapacityCaps = { staff: number, repairs: number, access: number, };

export type Ministry = "finance" | "education" | "infrastructure";

export type Family = "staffing" | "repairs" | "access";

export type Vote = "support" | "conditional" | "oppose";

export type Condition = { kind_id: string, family: Family, units: number, due_quarter: number, };

export type PlayerAction = { "kind": "propose", meeting_id: string, allocation: Allocation, } | { "kind": "amend", meeting_id: string, proposal_version: string, allocation: Allocation, } | { "kind": "ratify", meeting_id: string, proposal_version: string, } | { "kind": "request_capacity_cap", meeting_id: string, caps: CapacityCaps, } | { "kind": "order_audit", family: Family, inspected_quarter: number, } | { "kind": "disclose_evidence", evidence_id: string, evidence_version: string, recipients: Array<Ministry>, } | { "kind": "defer", } | { "kind": "advance_quarter", };

export type CommandEnvelope = { schema_version: number, campaign_id: string, command_id: string, expected_world_version: string, command: PlayerAction, };

export type Stage = "first" | "final";

export type Counterproposal = { "kind": "menu", menu_id: string, } | { "kind": "new_typed", };

export type MinisterAction = { "stage": "first", schema_version: number, meeting_id: string, proposal_version: string, ministry: Ministry, position: Vote, preferred_allocation: Allocation, counterproposal_source: Counterproposal, conditions: Array<Condition>, evidence_ids: Array<string>, rationale_summary: string, } | { "stage": "final", schema_version: number, meeting_id: string, proposal_version: string, ministry: Ministry, position: Vote, preferred_allocation: Allocation, conditions: Array<Condition>, evidence_ids: Array<string>, rationale_summary: string, implementation_effort_permille?: number, };

export type PublicCommitment = { id: string, issuer: Ministry, beneficiary: Ministry, proposal_version: string, kind: string, funded_units: number, trigger: string, deadline_quarter: number, status: string, resolution_event?: string, };

export type PublicRoleView = { schema_version: number, campaign_id: string, meeting_id: string, proposal_version: string, ministry: Ministry, quarter: number, allocation: Allocation, available_credits: number, belief_ppm: number, owned_commitments: Array<PublicCommitment>, visible_evidence_ids: Array<string>, };

export type PreviewKind = "optimistic_upper_bound";

export type PreviewCalculator = "myth_domain";

export type ForecastLine = { family: Family, per_quarter: [number, number, number, number, number, number], outstanding: number, };

export type ForecastResponse = { kind: PreviewKind, calculator: PreviewCalculator, authoritative_simulation: boolean, conditional_access_bonus: boolean, families: Array<ForecastLine>, };

export type ScriptedVoteKind = "clean_belief_scripted_votes";

export type ScriptedSeatVote = { ministry: Ministry, position: Vote, };

export type ScriptedVotes = { kind: ScriptedVoteKind, passes_support_threshold: boolean, institutional_ratification: boolean, votes: [ScriptedSeatVote, ScriptedSeatVote, ScriptedSeatVote], support_count: number, warning: string, };
