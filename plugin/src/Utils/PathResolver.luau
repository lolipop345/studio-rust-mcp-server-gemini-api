--!strict
-- PathResolver: resolves dot-separated path strings to Roblox Instances
-- Example: "Workspace.Folder1.Part" → game.Workspace.Folder1.Part

local PathResolver = {}

function PathResolver.resolve(path: string, createIfMissing: boolean?): Instance
	local segments = string.split(path, ".")
	if #segments == 0 then
		error("PathResolver: empty path")
	end

	local current: Instance = game:FindFirstChild(segments[1]) :: Instance
	if not current then
		error("PathResolver: service/root not found: '" .. segments[1] .. "' (full path: " .. path .. ")")
	end

	for i = 2, #segments do
		local child = current:FindFirstChild(segments[i])
		if not child then
			if createIfMissing then
				local newFolder = Instance.new("Folder")
				newFolder.Name = segments[i]
				newFolder.Parent = current
				child = newFolder
			else
				error("PathResolver: instance not found at segment '" .. segments[i] .. "' (full path: " .. path .. ")")
			end
		end
		
		if child then
			current = child
		end
	end

	return current
end

function PathResolver.resolveParent(path: string): (Instance, string)
	local segments = string.split(path, ".")
	if #segments == 0 then
		error("PathResolver: empty path")
	end

	if #segments == 1 then
		return game :: Instance, segments[1]
	end

	local parentPath = table.concat(segments, ".", 1, #segments - 1)
	local parent = PathResolver.resolve(parentPath)
	return parent, segments[#segments]
end

function PathResolver.getFullPath(instance: Instance): string
	local parts = {}
	local current: Instance? = instance
	while current and current ~= game do
		table.insert(parts, 1, current.Name)
		current = current.Parent
	end
	return table.concat(parts, ".")
end

return PathResolver
