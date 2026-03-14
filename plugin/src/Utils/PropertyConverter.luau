--!strict
-- PropertyConverter: converts JSON values to Roblox types based on property name and value shape
-- Shared by AddInstance and AddJsonInstance

local PropertyConverter = {}

-- Property names that should map to Color3 when receiving a 3-element array
local COLOR_PROPERTIES: { [string]: boolean } = {
	Color = true,
	Color3 = true,
	BackgroundColor3 = true,
	BorderColor3 = true,
	TextColor3 = true,
	PlaceholderColor3 = true,
	ImageColor3 = true,
	CursorPosition = false, -- not a color
}

function PropertyConverter.convert(propName: string, value: any): any
	if type(value) == "table" then
		local len = #value

		if len == 2 then
			-- UDim or Vector2
			if string.find(propName, "Transparency") or string.find(propName, "Udim") then
				return UDim.new(value[1], value[2])
			end
			return Vector2.new(value[1], value[2])
		elseif len == 3 then
			-- Vector3 or Color3
			if COLOR_PROPERTIES[propName] then
				return Color3.new(value[1], value[2], value[3])
			end
			return Vector3.new(value[1], value[2], value[3])
		elseif len == 4 then
			-- UDim2
			return UDim2.new(value[1], value[2], value[3], value[4])
		end

		-- Fallback: return as-is
		return value
	elseif type(value) == "string" then
		-- Try to parse "EnumType.Value" format
		local enumType, enumValue = string.match(value, "^(%w+)%.(%w+)$")
		if enumType and enumValue then
			local enumGroup = (Enum :: any)[enumType]
			if enumGroup then
				local success, result = pcall(function()
					return (enumGroup :: any)[enumValue]
				end)
				if success and result then
					return result
				end
			end
		end
		return value
	end

	-- number, boolean, etc.
	return value
end

function PropertyConverter.applyProperties(instance: Instance, properties: { [string]: any })
	for propName, propValue in properties do
		local converted = PropertyConverter.convert(propName, propValue)
		local success, err = pcall(function()
			(instance :: any)[propName] = converted
		end)
		if not success then
			warn("PropertyConverter: failed to set " .. propName .. " on " .. instance.ClassName .. ": " .. tostring(err))
		end
	end
end

return PropertyConverter
