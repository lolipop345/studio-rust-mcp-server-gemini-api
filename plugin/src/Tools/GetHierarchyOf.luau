local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)
local HttpService = game:GetService("HttpService")

local function serializeInstance(instance: Instance, depth: number, maxDepth: number, includeBaseParts: boolean): any
	local node: { [string]: any } = {
		Name = instance.Name,
		ClassName = instance.ClassName,
	}

	if depth >= maxDepth then
		local childCount = #instance:GetChildren()
		if childCount > 0 then
			node.ChildCount = childCount
		end
		return node
	end

	local children = instance:GetChildren()
	if #children == 0 then
		return node
	end

	local serializedChildren = {}
	local basePartCount = 0

	for _, child in children do
		if child:IsA("BasePart") and not includeBaseParts then
			basePartCount += 1
		else
			table.insert(serializedChildren, serializeInstance(child, depth + 1, maxDepth, includeBaseParts))
		end
	end

	if #serializedChildren > 0 then
		node.Children = serializedChildren
	end

	if basePartCount > 0 then
		node.BasePartCount = basePartCount
	end

	return node
end

local function handleGetHierarchyOf(args: Types.GetHierarchyOfArgs): string?
	if type(args.path) ~= "string" then
		error("GetHierarchyOf: missing 'path'")
	end

	local instance = PathResolver.resolve(args.path)
	local includeBaseParts = args.includeBaseParts or false

	local result = serializeInstance(instance, 0, 10, includeBaseParts)
	return HttpService:JSONEncode(result)
end

return handleGetHierarchyOf :: Types.ToolFunction
