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

	-- Security: URL validation (SSRF protection)
	local url = args.url
	local lowerUrl = string.lower(url)

	-- Must be HTTPS (except raw.githubusercontent.com which redirects)
	if not (string.sub(lowerUrl, 1, 8) == "https://") then
		error("ImportFromHttp: Only HTTPS URLs are allowed for security. Got: " .. string.sub(url, 1, 30))
	end

	-- Block internal/private network URLs
	local blockedPatterns = {
		"://localhost", "://127%.", "://10%.", "://192%.168%.",
		"://172%.1[6-9]%.", "://172%.2%d%.", "://172%.3[0-1]%.",
		"://0%.0%.0%.0", "://::1", "://169%.254%.",
		"://metadata", "://internal",
	}
	for _, pattern in ipairs(blockedPatterns) do
		if string.find(lowerUrl, pattern) then
			error("ImportFromHttp: URL points to a private/internal address. This is blocked for security.")
		end
	end

	-- Only allow known safe domains for code imports
	local ALLOWED_DOMAINS = {
		"raw.githubusercontent.com",
		"gist.githubusercontent.com",
		"github.com",
		"gitlab.com",
		"bitbucket.org",
		"pastebin.com",
		"cdn.jsdelivr.net",
		"unpkg.com",
		"raw.githubusercontents.com",
	}
	local domain = string.match(lowerUrl, "https://([^/]+)")
	local domainAllowed = false
	for _, allowed in ipairs(ALLOWED_DOMAINS) do
		if domain == allowed or string.sub(domain, -(#allowed + 1)) == "." .. allowed then
			domainAllowed = true
			break
		end
	end
	if not domainAllowed then
		error("ImportFromHttp: Domain '" .. (domain or "unknown") .. "' is not in the allowed list. Allowed: GitHub, GitLab, Bitbucket, Pastebin, jsDelivr, unpkg.")
	end

	local parentInstance = PathResolver.resolve(args.parent)

	local success, source = pcall(function()
		return HttpService:GetAsync(url)
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
