local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)

local function handleReadLines(args: Types.ReadLinesArgs): string?
	if type(args.path) ~= "string" then
		error("ReadLines: missing 'path'")
	end

	local instance = PathResolver.resolve(args.path)

	if not instance:IsA("LuaSourceContainer") then
		error("ReadLines: instance at '" .. args.path .. "' is not a script (got " .. instance.ClassName .. ")")
	end

	local source = (instance :: any).Source :: string
	local lines = string.split(source, "\n")

	local startLine = math.max(1, args.startLine or 1)
	local endLine = math.min(#lines, args.endLine or #lines)

	local result = {}
	for i = startLine, endLine do
		table.insert(result, string.format("%4d | %s", i, lines[i] or ""))
	end

	return table.concat(result, "\n")
end

return handleReadLines :: Types.ToolFunction
