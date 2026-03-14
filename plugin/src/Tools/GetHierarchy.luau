local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local HttpService = game:GetService("HttpService")

local TOP_LEVEL_SERVICES = {
	"Workspace",
	"ServerScriptService",
	"ServerStorage",
	"ReplicatedStorage",
	"ReplicatedFirst",
	"StarterGui",
	"StarterPack",
	"StarterPlayer",
	"Lighting",
	"SoundService",
	"Chat",
	"Teams",
}

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

local function handleGetHierarchy(args: Types.GetHierarchyArgs): string?
	local includeBaseParts = args.includeBaseParts or false
	local maxDepth = args.maxDepth or 3

	local result = {}

	for _, serviceName in TOP_LEVEL_SERVICES do
		local service = game:FindFirstChild(serviceName)
		if service then
			table.insert(result, serializeInstance(service, 0, maxDepth, includeBaseParts))
		end
	end

	return HttpService:JSONEncode(result)
end

return handleGetHierarchy :: Types.ToolFunction
