local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)

local tools: { [string]: Types.ToolFunction } = {
	["GetConsoleOutput"] = require(Main.Tools.GetConsoleOutput),
	["GetStudioMode"] = require(Main.Tools.GetStudioMode),
	["InsertModel"] = require(Main.Tools.InsertModel),
	["RunCode"] = require(Main.Tools.RunCode),
	["RunScriptInPlayMode"] = require(Main.Tools.RunScriptInPlayMode),
	["StartStopPlay"] = require(Main.Tools.StartStopPlay),
	-- Agent Tools
	["ReadFile"] = require(Main.Tools.ReadFile),
	["ReadLines"] = require(Main.Tools.ReadLines),
	["GetHierarchy"] = require(Main.Tools.GetHierarchy),
	["GetHierarchyOf"] = require(Main.Tools.GetHierarchyOf),
	["GetProperties"] = require(Main.Tools.GetProperties),
	["FindInstances"] = require(Main.Tools.FindInstances),
	["AddInstance"] = require(Main.Tools.AddInstance),
	["RemoveInstance"] = require(Main.Tools.RemoveInstance),
	["AddJsonInstance"] = require(Main.Tools.AddJsonInstance),
	["ReplaceLinesWith"] = require(Main.Tools.ReplaceLinesWith),
	["ReplaceWith"] = require(Main.Tools.ReplaceWith),
	["MoveInstance"] = require(Main.Tools.MoveInstance),
	["CloneInstance"] = require(Main.Tools.CloneInstance),
	["ImportFromHttp"] = require(Main.Tools.ImportFromHttp),
	["ImportFromWally"] = require(Main.Tools.ImportFromWally),
	["CreateScript"] = require(Main.Tools.CreateScript),
	["DebugScript"] = require(Main.Tools.DebugScript),
}

local function dispatchTool(toolName: string, args: Types.ToolArgs): string?
	local toolFunction = tools[toolName]
	if not toolFunction then
		error("Tool " .. toolName .. " not found")
	end

	return toolFunction(args)
end

return {
	tools = tools,
	dispatchTool = dispatchTool,
}
