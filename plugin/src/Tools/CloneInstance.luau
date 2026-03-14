local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)

local function handleCloneInstance(args: Types.CloneInstanceArgs): string?
	if type(args.path) ~= "string" then
		error("CloneInstance: missing 'path'")
	end

	local instance = PathResolver.resolve(args.path)
	local clone = instance:Clone()

	if args.newParent and type(args.newParent) == "string" then
		local newParent = PathResolver.resolve(args.newParent)
		clone.Parent = newParent
	else
		clone.Parent = instance.Parent
	end

	return "Cloned " .. args.path .. " → " .. PathResolver.getFullPath(clone)
end

return handleCloneInstance :: Types.ToolFunction
