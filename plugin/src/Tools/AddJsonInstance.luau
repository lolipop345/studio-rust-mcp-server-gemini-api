local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)
local PropertyConverter = require(Main.Utils.PropertyConverter)
local HttpService = game:GetService("HttpService")

local function buildInstanceTree(node: any, parent: Instance)
	if type(node) ~= "table" then
		error("AddJsonInstance: invalid node in JSON tree")
	end

	local className = node.ClassName
	if type(className) ~= "string" then
		error("AddJsonInstance: each node must have a 'ClassName' field")
	end

	local instance = Instance.new(className)

	if node.Name and type(node.Name) == "string" then
		instance.Name = node.Name
	end

	if node.Properties and type(node.Properties) == "table" then
		PropertyConverter.applyProperties(instance, node.Properties)
	end

	-- Recursively build children before parenting
	if node.Children and type(node.Children) == "table" then
		for _, childNode in node.Children do
			buildInstanceTree(childNode, instance)
		end
	end

	instance.Parent = parent
end

local function handleAddJsonInstance(args: Types.AddJsonInstanceArgs): string?
	if args.json == nil then
		error("AddJsonInstance: missing 'json'")
	end
	if type(args.parent) ~= "string" then
		error("AddJsonInstance: missing 'parent'")
	end

	local parentInstance = PathResolver.resolve(args.parent)

	-- args.json could be a table (already parsed) or a string (needs parsing)
	local tree = args.json
	if type(tree) == "string" then
		tree = HttpService:JSONDecode(tree)
	end

	buildInstanceTree(tree, parentInstance)

	return "Created instance tree under " .. args.parent
end

return handleAddJsonInstance :: Types.ToolFunction
