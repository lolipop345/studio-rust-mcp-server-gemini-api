local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)

local function handleMoveInstance(args: Types.MoveInstanceArgs): string?
	if type(args.path) ~= "string" then
		error("MoveInstance: missing 'path'")
	end
	if type(args.newParent) ~= "string" then
		error("MoveInstance: missing 'newParent'")
	end

	local instance = PathResolver.resolve(args.path)
	local newParent = PathResolver.resolve(args.newParent)

	local oldPath = PathResolver.getFullPath(instance)
	instance.Parent = newParent
	local newPath = PathResolver.getFullPath(instance)

	return "Moved " .. oldPath .. " → " .. newPath
end

return handleMoveInstance :: Types.ToolFunction
