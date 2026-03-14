local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)
local PropertyConverter = require(Main.Utils.PropertyConverter)

local function handleAddInstance(args: Types.AddInstanceArgs): string?
	if type(args.className) ~= "string" then
		error("AddInstance: missing 'className'")
	end
	if type(args.parent) ~= "string" then
		error("AddInstance: missing 'parent'")
	end

	local parentInstance = PathResolver.resolve(args.parent)

	local instance = Instance.new(args.className)

	if args.properties then
		PropertyConverter.applyProperties(instance, args.properties)
	end

	instance.Parent = parentInstance

	return "Created " .. args.className .. " at " .. PathResolver.getFullPath(instance)
end

return handleAddInstance :: Types.ToolFunction
