local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)

local function handleReadFile(args: Types.ReadFileArgs): string?
	if type(args.path) ~= "string" then
		error("ReadFile: missing 'path'")
	end

	local instance = PathResolver.resolve(args.path)

	if not instance:IsA("LuaSourceContainer") then
		error("ReadFile: instance at '" .. args.path .. "' is not a script (got " .. instance.ClassName .. ")")
	end

	return (instance :: any).Source
end

return handleReadFile :: Types.ToolFunction
