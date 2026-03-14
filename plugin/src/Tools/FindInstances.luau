local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)
local HttpService = game:GetService("HttpService")

local MAX_RESULTS = 50

local function handleFindInstances(args: Types.FindInstancesArgs): string?
	if not args.name and not args.className then
		error("FindInstances: at least one of 'name' or 'className' must be provided")
	end

	local root: Instance
	if args.root and args.root ~= "" and args.root ~= "game" then
		root = PathResolver.resolve(args.root)
	else
		root = game :: Instance
	end

	local namePattern = args.name and string.lower(args.name) or nil
	local descendants = root:GetDescendants()

	local results = {}

	for _, instance in descendants do
		if #results >= MAX_RESULTS then
			break
		end

		local nameMatch = true
		local classMatch = true

		if namePattern then
			nameMatch = string.find(string.lower(instance.Name), namePattern, 1, true) ~= nil
		end

		if args.className then
			classMatch = instance.ClassName == args.className
		end

		if nameMatch and classMatch then
			table.insert(results, {
				path = PathResolver.getFullPath(instance),
				name = instance.Name,
				className = instance.ClassName,
			})
		end
	end

	return HttpService:JSONEncode(results)
end

return handleFindInstances :: Types.ToolFunction
