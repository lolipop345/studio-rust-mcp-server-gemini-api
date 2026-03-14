local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)

local VALID_TYPES: { [string]: boolean } = {
	["Script"] = true,
	["LocalScript"] = true,
	["ModuleScript"] = true,
}

local function handleCreateScript(args: any): string?
	local scriptType = args.scriptType
	local name = args.name
	local source = args.source
	local parent = args.parent

	if type(scriptType) ~= "string" or not VALID_TYPES[scriptType] then
		error("CreateScript: invalid 'scriptType'. Must be Script, LocalScript, or ModuleScript.")
	end
	if type(name) ~= "string" or name == "" then
		error("CreateScript: missing or empty 'name'.")
	end
	if type(source) ~= "string" then
		error("CreateScript: missing 'source'.")
	end
	if type(parent) ~= "string" then
		error("CreateScript: missing 'parent'.")
	end

	local parentInstance = PathResolver.resolve(parent)

	local success, instance = pcall(Instance.new, scriptType)
	if not success or not instance then
		error("CreateScript: failed to create instance of type '" .. scriptType .. "'.")
	end

	instance.Name = name

	pcall(function()
		(instance :: any).Source = source
	end)

	instance.Parent = parentInstance

	return "Created " .. scriptType .. " '" .. name .. "' under " .. parent
end

return handleCreateScript :: Types.ToolFunction
