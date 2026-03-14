local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)
local PropertyConverter = require(Main.Utils.PropertyConverter)
local HttpService = game:GetService("HttpService")

-- ImportFromWally: the backend handles wally install and sends a JSON file tree
-- This plugin handler receives the pre-built file tree and creates ModuleScript instances

local function buildFileTree(node: any, parent: Instance, fallbackName: string?)
	if type(node) ~= "table" then
		return
	end

	local className = node.ClassName or "ModuleScript"
	local name = node.Name or fallbackName or "Module"
	
	-- Try to merge into an existing instance if it's a structural Folder or matches class
	local instance = parent:FindFirstChild(name)
	if not instance or instance.ClassName ~= className then
		local success, newInstance = pcall(Instance.new, className)
		if not success then
			newInstance = Instance.new("Folder")
		end
		instance = newInstance
		instance.Name = name
		instance.Parent = parent
	end

	if node.Source and type(node.Source) == "string" then
		pcall(function()
			(instance :: any).Source = node.Source
		end)
	end

	if node.Children and type(node.Children) == "table" then
		for key, child in node.Children do
			local childFallbackName = if type(key) == "string" then key else nil
			buildFileTree(child, instance, childFallbackName)
		end
	end
end

local function handleImportFromWally(args: Types.ImportFromWallyArgs): string?
	if type(args.package) ~= "string" then
		error("ImportFromWally: missing 'package'")
	end
	if type(args.parent) ~= "string" then
		error("ImportFromWally: missing 'parent'")
	end

	-- The args.json field contains the pre-built file tree from the backend
	-- This is injected by the backend's special Wally handler
	local argsAny = args :: any
	if argsAny.json == nil then
		error("ImportFromWally: no file tree received from backend. Is Wally installed?")
	end

	local parentInstance = PathResolver.resolve(args.parent, true)

	local tree = argsAny.json
	if type(tree) == "string" then
		tree = HttpService:JSONDecode(tree)
	end

	if type(tree) == "table" and tree.files then
		-- Multiple files in a flat list
		for _, fileNode in tree.files do
			buildFileTree(fileNode, parentInstance)
		end
	elseif type(tree) == "table" and #tree > 0 then
		-- Array of top-level nodes
		for _, fileNode in ipairs(tree) do
			buildFileTree(fileNode, parentInstance)
		end
	else
		-- Single tree node
		buildFileTree(tree, parentInstance)
	end

	return "Imported Wally package '" .. args.package .. "' into " .. args.parent
end

return handleImportFromWally :: Types.ToolFunction
