local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)

local function handleRemoveInstance(args: Types.RemoveInstanceArgs): string?
	if type(args.path) ~= "string" then
		error("RemoveInstance: missing 'path'")
	end

	local instance = PathResolver.resolve(args.path)
	local fullPath = PathResolver.getFullPath(instance)
	local className = instance.ClassName

	instance:Destroy()

	return "Removed " .. className .. " at " .. fullPath
end

return handleRemoveInstance :: Types.ToolFunction
