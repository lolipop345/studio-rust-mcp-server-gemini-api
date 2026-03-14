local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)

local function handleReplaceWith(args: Types.ReplaceWithArgs): string?
	if type(args.path) ~= "string" then
		error("ReplaceWith: missing 'path'")
	end
	if type(args.newSource) ~= "string" then
		error("ReplaceWith: missing 'newSource'")
	end

	local instance = PathResolver.resolve(args.path)

	if not instance:IsA("LuaSourceContainer") then
		error("ReplaceWith: instance at '" .. args.path .. "' is not a script (got " .. instance.ClassName .. ")")
	end

	(instance :: any).Source = args.newSource

	local lineCount = #string.split(args.newSource, "\n")
	return string.format("Replaced entire source of %s (%d lines)", args.path, lineCount)
end

return handleReplaceWith :: Types.ToolFunction
