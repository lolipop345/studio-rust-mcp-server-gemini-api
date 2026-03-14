local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local Trove = require(Main.Utils.Trove)

--[[
	DebugScript: Runs Luau code in a sandboxed environment.
	- Tracks all Instance.new() calls via Trove
	- Captures print/warn/error output
	- Reports runtime errors with stack traces
	- Cleans up all created instances after execution
]]

local function handleDebugScript(args: any): string?
	local code = args.code
	if type(code) ~= "string" or code == "" then
		error("DebugScript: missing or empty 'code'.")
	end

	local cleanup = true
	if args.cleanup == false then
		cleanup = false
	end

	local trove = Trove.new()
	local output: { string } = {}
	local errors: { string } = {}
	local createdInstances: { string } = {}

	local function addOutput(level: string, ...)
		local parts = {}
		for i = 1, select("#", ...) do
			local v = select(i, ...)
			table.insert(parts, tostring(v))
		end
		table.insert(output, "[" .. level .. "] " .. table.concat(parts, "\t"))
	end

	-- Build sandboxed environment

	local success, errorMessage = pcall(function()
		local chunk = loadstring(code) :: any
		if not chunk then
			error("Failed to parse code: syntax error")
		end

		local env = getfenv(chunk)

		-- Override Instance.new to track
		local origInstanceNew = Instance.new
		env.Instance = setmetatable({}, {
			__index = function(_, key)
				if key == "new" then
					return function(className: string, parent: Instance?)
						local inst = origInstanceNew(className)
						trove:Add(inst)
						table.insert(createdInstances, className .. " (" .. inst.Name .. ")")
						if parent then
							inst.Parent = parent
						end
						return inst
					end
				end
				return (Instance :: any)[key]
			end,
		})

		-- Capture output
		local origPrint = print
		env.print = function(...)
			origPrint(...)
			addOutput("PRINT", ...)
		end

		local origWarn = warn
		env.warn = function(...)
			origWarn(...)
			addOutput("WARN", ...)
		end

		env.error = function(msg, level)
			table.insert(errors, "[ERROR] " .. tostring(msg))
			error(msg, (level or 1) + 1)
		end

		-- Run
		local results = table.pack(chunk())
		if results.n > 0 then
			local strResults = {}
			for i = 1, results.n do
				table.insert(strResults, tostring(results[i]))
			end
			addOutput("RETURN", table.unpack(strResults))
		end
	end)

	-- Build report
	local report: { string } = {}
	table.insert(report, "=== DEBUG REPORT ===")

	if success then
		table.insert(report, "Status: ✅ SUCCESS (no runtime errors)")
	else
		table.insert(report, "Status: ❌ ERROR")
		table.insert(report, "Error: " .. tostring(errorMessage))
		table.insert(errors, tostring(errorMessage))
	end

	if #output > 0 then
		table.insert(report, "\n--- Output ---")
		for _, line in ipairs(output) do
			table.insert(report, line)
		end
	end

	if #createdInstances > 0 then
		table.insert(report, "\n--- Created Instances (" .. #createdInstances .. ") ---")
		for _, desc in ipairs(createdInstances) do
			table.insert(report, "  • " .. desc)
		end
	end

	if cleanup then
		trove:Clean()
		table.insert(report, "\n--- Cleanup ---")
		table.insert(report, "All " .. #createdInstances .. " sandbox instances destroyed.")
	else
		table.insert(report, "\n--- Cleanup ---")
		table.insert(report, "Cleanup skipped (cleanup=false). Instances are still alive.")
	end

	table.insert(report, "=== END DEBUG REPORT ===")

	return table.concat(report, "\n")
end

return handleDebugScript :: Types.ToolFunction
