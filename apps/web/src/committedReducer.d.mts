import type {Committed} from './campaign'
export function newer(old:Committed | null,next:Committed):Committed

export function validCommittedCursor(value:unknown):value is string

export function validSnapshot(raw:unknown,campaignId:string):raw is Committed
