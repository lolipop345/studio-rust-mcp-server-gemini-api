local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)

local function handleReplaceLinesWith(args: Types.ReplaceLinesWithArgs): string?
	if type(args.path) ~= "string" then
		error("ReplaceLinesWith: missing 'path'")
	end

	local instance = PathResolver.resolve(args.path)

	if not instance:IsA("LuaSourceContainer") then
		error("ReplaceLinesWith: instance at '" .. args.path .. "' is not a script (got " .. instance.ClassName .. ")")
	end

	local source = (instance :: any).Source :: string
	local lines = string.split(source, "\n")

	local startLine = math.max(1, args.startLine or 1)
	local endLine = math.min(#lines, args.endLine or #lines)

	-- Build new source: lines before + new content + lines after
	local newLines = {}

	for i = 1, startLine - 1 do
		table.insert(newLines, lines[i])
	end

	-- Insert new content lines
	local contentLines = string.split(args.newContent, "\n")
	for _, line in contentLines do
		table.insert(newLines, line)
	end

	for i = endLine + 1, #lines do
		table.insert(newLines, lines[i])
	end

	(instance :: any).Source = table.concat(newLines, "\n")

	local linesReplaced = endLine - startLine + 1
	local linesInserted = #contentLines
	return string.format("Replaced lines %d-%d (%d lines) with %d new lines in %s", startLine, endLine, linesReplaced, linesInserted, args.path)
end

return handleReplaceLinesWith :: Types.ToolFunction
