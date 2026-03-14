local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)
local HttpService = game:GetService("HttpService")

local VALID_TYPES = {
	Script = true,
	LocalScript = true,
	ModuleScript = true,
}

local function handleImportFromHttp(args: Types.ImportFromHttpArgs): string?
	if type(args.url) ~= "string" then
		error("ImportFromHttp: missing 'url'")
	end
	if type(args.parent) ~= "string" then
		error("ImportFromHttp: missing 'parent'")
	end
	if type(args.instanceType) ~= "string" or not VALID_TYPES[args.instanceType] then
		error("ImportFromHttp: 'instanceType' must be Script, LocalScript, or ModuleScript")
	end
	if type(args.name) ~= "string" then
		error("ImportFromHttp: missing 'name'")
	end

	-- Check HttpEnabled
	local httpEnabled = HttpService.HttpEnabled
	if not httpEnabled then
		error("ImportFromHttp: HttpService.HttpEnabled is false. Enable it in Game Settings → Security.")
	end

	local parentInstance = PathResolver.resolve(args.parent)

	local success, source = pcall(function()
		return HttpService:GetAsync(args.url)
	end)

	if not success then
		error("ImportFromHttp: failed to fetch URL: " .. tostring(source))
	end

	local instance = Instance.new(args.instanceType)
	instance.Name = args.name;
	(instance :: any).Source = source
	instance.Parent = parentInstance

	local lineCount = #string.split(source, "\n")
	return string.format("Imported %s '%s' from URL (%d lines) → %s", args.instanceType, args.name, lineCount, PathResolver.getFullPath(instance))
end

return handleImportFromHttp :: Types.ToolFunction
