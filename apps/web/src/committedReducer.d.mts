import type {Committed} from './views/ConnectedView'
export function newer(old:Committed | null,next:Committed):Committed

export function validCommittedCursor(value:unknown):value is string
