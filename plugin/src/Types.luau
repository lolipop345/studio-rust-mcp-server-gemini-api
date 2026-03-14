export type InsertModelArgs = {
	query: string,
}

export type RunCodeArgs = {
	command: string,
}

export type TestMode = "start_play" | "run_server"

export type StartStopPlayMode = TestMode | "stop"

export type StartStopPlayArgs = {
	mode: StartStopPlayMode,
}

export type RunScriptInPlayModeArgs = {
	code: string,
	timeout: number?,
	mode: TestMode,
}

export type ReadFileArgs = {
	path: string,
}

export type ReadLinesArgs = {
	path: string,
	startLine: number,
	endLine: number,
}

export type GetHierarchyArgs = {
	includeBaseParts: boolean?,
	maxDepth: number?,
}

export type GetHierarchyOfArgs = {
	path: string,
	includeBaseParts: boolean?,
}

export type GetPropertiesArgs = {
	path: string,
	properties: { string }?,
}

export type FindInstancesArgs = {
	name: string?,
	className: string?,
	root: string?,
}

export type AddInstanceArgs = {
	className: string,
	parent: string,
	properties: { [string]: any }?,
}

export type RemoveInstanceArgs = {
	path: string,
}

export type AddJsonInstanceArgs = {
	json: any,
	parent: string,
}

export type ReplaceLinesWithArgs = {
	path: string,
	startLine: number,
	endLine: number,
	newContent: string,
}

export type ReplaceWithArgs = {
	path: string,
	newSource: string,
}

export type MoveInstanceArgs = {
	path: string,
	newParent: string,
}

export type CloneInstanceArgs = {
	path: string,
	newParent: string?,
}

export type ImportFromHttpArgs = {
	url: string,
	parent: string,
	instanceType: string,
	name: string,
}

export type ImportFromWallyArgs = {
	package: string,
	parent: string,
}

export type ToolArgs =
	{}
	| { InsertModel: InsertModelArgs }
	| { RunCode: RunCodeArgs }
	| { StartStopPlay: StartStopPlayArgs }
	| { RunScriptInPlayMode: RunScriptInPlayModeArgs }
	| { ReadFile: ReadFileArgs }
	| { ReadLines: ReadLinesArgs }
	| { GetHierarchy: GetHierarchyArgs }
	| { GetHierarchyOf: GetHierarchyOfArgs }
	| { GetProperties: GetPropertiesArgs }
	| { FindInstances: FindInstancesArgs }
	| { AddInstance: AddInstanceArgs }
	| { RemoveInstance: RemoveInstanceArgs }
	| { AddJsonInstance: AddJsonInstanceArgs }
	| { ReplaceLinesWith: ReplaceLinesWithArgs }
	| { ReplaceWith: ReplaceWithArgs }
	| { MoveInstance: MoveInstanceArgs }
	| { CloneInstance: CloneInstanceArgs }
	| { ImportFromHttp: ImportFromHttpArgs }
	| { ImportFromWally: ImportFromWallyArgs }

export type ToolFunction = (ToolArgs) -> string?

return {}
