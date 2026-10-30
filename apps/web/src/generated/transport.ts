// GENERATED from myth-contracts. DO NOT EDIT.
// All *_version and expected_world_version fields are decimal strings.

export type Allocation = { staff: number, repairs: number, access: number, };

export type CapacityCaps = { staff: number, repairs: number, access: number, };

export type Ministry = "finance" | "education" | "infrastructure";

export type Family = "staffing" | "repairs" | "access";

export type Vote = "support" | "conditional" | "oppose";

export type Condition = { kind_id: string, family: Family, units: number, due_quarter: number, };

export type PlayerAction = { "kind": "propose", meeting_id: string, allocation: Allocation, } | { "kind": "amend", meeting_id: string, proposal_version: string, allocation: Allocation, } | { "kind": "ratify", meeting_id: string, proposal_version: string, } | { "kind": "request_capacity_cap", meeting_id: string, caps: CapacityCaps, } | { "kind": "order_audit", family: Family, inspected_quarter: number, } | { "kind": "disclose_evidence", evidence_id: string, evidence_version: string, recipients: Array<Ministry>, } | { "kind": "defer" } | { "kind": "advance_quarter" };

export type CommandEnvelope = { schema_version: number, campaign_id: string, command_id: string, expected_world_version: string, command: PlayerAction, };

export type Stage = "first" | "final";

export type Counterproposal = { "kind": "menu", menu_id: string, } | { "kind": "new_typed" };

export type MinisterAction = { schema_version: number, meeting_id: string, proposal_version: string, ministry: Ministry, stage: Stage, position: Vote, preferred_allocation: Allocation, counterproposal_source: Counterproposal, conditions: Array<Condition>, evidence_ids: Array<string>, rationale_summary: string, implementation_effort_permille: number | null, };

export type PublicRoleView = { schema_version: number, campaign_id: string, meeting_id: string, proposal_version: string, ministry: Ministry, quarter: number, allocation: Allocation, available_credits: number, belief_ppm: number, visible_evidence_ids: Array<string>, };
