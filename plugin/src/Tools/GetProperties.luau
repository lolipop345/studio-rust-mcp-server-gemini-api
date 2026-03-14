local Main = script:FindFirstAncestor("MCPStudioPlugin")
local Types = require(Main.Types)
local PathResolver = require(Main.Utils.PathResolver)
local HttpService = game:GetService("HttpService")

local COMMON_PROPERTIES = {
	"Name", "ClassName", "Parent",
	-- BasePart
	"Position", "Size", "CFrame", "Anchored", "CanCollide", "Transparency", "Color", "Material", "Shape",
	-- GuiObject
	"BackgroundColor3", "BackgroundTransparency", "BorderColor3", "BorderSizePixel",
	"AnchorPoint", "Visible", "ZIndex", "LayoutOrder",
	-- Text
	"Text", "TextColor3", "TextSize", "Font", "TextScaled",
	-- Script
	"Source", "Enabled", "RunContext",
	-- Model
	"PrimaryPart",
	-- Other
	"Value", "MaxValue", "MinValue",
}

local function serializeValue(value: any): any
	local t = typeof(value)

	if t == "Vector3" then
		return { value.X, value.Y, value.Z }
	elseif t == "Vector2" then
		return { value.X, value.Y }
	elseif t == "Color3" then
		return { math.floor(value.R * 255), math.floor(value.G * 255), math.floor(value.B * 255) }
	elseif t == "UDim2" then
		return { value.X.Scale, value.X.Offset, value.Y.Scale, value.Y.Offset }
	elseif t == "UDim" then
		return { value.Scale, value.Offset }
	elseif t == "CFrame" then
		return tostring(value)
	elseif t == "BrickColor" then
		return value.Name
	elseif t == "EnumItem" then
		return tostring(value)
	elseif t == "Instance" then
		return PathResolver.getFullPath(value)
	elseif t == "number" or t == "string" or t == "boolean" then
		return value
	else
		return tostring(value)
	end
end

local function handleGetProperties(args: Types.GetPropertiesArgs): string?
	if type(args.path) ~= "string" then
		error("GetProperties: missing 'path'")
	end

	local instance = PathResolver.resolve(args.path)
	local propList = args.properties or COMMON_PROPERTIES

	local result: { [string]: any } = {}

	for _, propName in propList do
		local success, value = pcall(function()
			return (instance :: any)[propName]
		end)
		if success and value ~= nil then
			result[propName] = serializeValue(value)
		end
	end

	return HttpService:JSONEncode(result)
end

return handleGetProperties :: Types.ToolFunction
