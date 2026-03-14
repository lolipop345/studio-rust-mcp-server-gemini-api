local TweenService = game:GetService("TweenService")
local CoreGui = game:GetService("CoreGui")

local AgentActivityUI = {
	isBackendWorking = false,
	activeTool = nil,
	backendTool = nil,
}

local currentScreenGui: ScreenGui? = nil
local currentBorder: ImageLabel? = nil
local currentLoopConnection: RBXScriptConnection? = nil

local GOOGLE_COLORS = ColorSequence.new({
	ColorSequenceKeypoint.new(0, Color3.fromRGB(66, 133, 244)),
	ColorSequenceKeypoint.new(0.33, Color3.fromRGB(234, 67, 53)),
	ColorSequenceKeypoint.new(0.66, Color3.fromRGB(251, 188, 5)),
	ColorSequenceKeypoint.new(1, Color3.fromRGB(66, 133, 244)),
})

local activePulseTween: Tween? = nil

local function stopPulse()
	if activePulseTween then
		pcall(function() activePulseTween:Cancel() end)
		activePulseTween = nil
	end
end

local function tweenElements(elements: any)
	local tweenInfo = TweenInfo.new(0.5, Enum.EasingStyle.Quad, Enum.EasingDirection.Out)
	for element, targetProps in pairs(elements) do
		if element then
			local tween = TweenService:Create(element, tweenInfo, targetProps)
			tween:Play()
		end
	end
end

local function buildUI()
	local screenGui = Instance.new("ScreenGui")
	screenGui.Name = "MCPAgentActivityUI"
	screenGui.Parent = CoreGui

	local border = Instance.new("ImageLabel")
	border.Name = "GradientBorder"
	border.BackgroundTransparency = 1
	border.ImageTransparency = 1 -- Start transparent
	border.Image = "rbxassetid://18720640102"
	border.ScaleType = Enum.ScaleType.Slice
	border.SliceCenter = Rect.new(384, 384, 384, 384)
	border.SliceScale = 0.3
	border.Size = UDim2.new(1, 0, 1, 0)
	border.Position = UDim2.new(0, 0, 0, 0)
	border.ZIndex = 1

	local uiGradient = Instance.new("UIGradient")
	uiGradient.Color = GOOGLE_COLORS
	uiGradient.Parent = border
	border.Parent = screenGui

	return screenGui, border, uiGradient
end

local function _internalShow(toolName: string?)
	if not currentScreenGui then
		local screenGui, border, gradient = buildUI()
		currentScreenGui = screenGui
		currentBorder = border
		
		-- Rotation loop (slower)
		local rotation = 0
		currentLoopConnection = game:GetService("RunService").RenderStepped:Connect(function(dt)
			rotation = (rotation + dt * 25) % 360
			gradient.Rotation = rotation
		end)

		-- Pulse SliceScale
		local pulseInfo = TweenInfo.new(2, Enum.EasingStyle.Sine, Enum.EasingDirection.InOut, -1, true)
		activePulseTween = TweenService:Create(border, pulseInfo, {SliceScale = 0.12})
		if activePulseTween then
			activePulseTween:Play()
		end
		
		-- Fade in entrance
		tweenElements({
			[border :: any] = { ImageTransparency = 0 } :: any
		} :: any)
	end
end

local function _internalHide()
	if not currentScreenGui then return end

	local uiToDestroy = currentScreenGui
	local border = currentBorder
	local loopConn = currentLoopConnection

	currentScreenGui = nil
	currentBorder = nil
	currentLoopConnection = nil
	
	if loopConn then
		loopConn:Disconnect()
	end

	stopPulse()

	-- Fade out exit
	local tweenInfo = TweenInfo.new(0.5, Enum.EasingStyle.Quad, Enum.EasingDirection.Out)
	local exitTweens = {}
	
	table.insert(exitTweens, TweenService:Create(border :: any, tweenInfo, { ImageTransparency = 1 }))
	
	for _, tw in ipairs(exitTweens) do
		tw:Play()
	end

	-- Wait then destroy
	task.delay(0.6, function()
		if uiToDestroy then
			uiToDestroy:Destroy()
		end
	end)
end

function AgentActivityUI.updateState()
	local shouldShow = AgentActivityUI.isBackendWorking or AgentActivityUI.activeTool ~= nil
	if shouldShow then
		-- Prefer the backend-reported tool, fall back to locally-set activeTool
		local displayTool = AgentActivityUI.backendTool or AgentActivityUI.activeTool
		_internalShow(displayTool)
	else
		_internalHide()
	end
end

function AgentActivityUI.setBackendWorking(isWorking: boolean, currentTool: string?)
	AgentActivityUI.isBackendWorking = isWorking
	AgentActivityUI.backendTool = currentTool
	AgentActivityUI.updateState()
end

function AgentActivityUI.setActiveTool(toolName: string?)
	AgentActivityUI.activeTool = toolName
	AgentActivityUI.updateState()
end

-- Backward compatibility for reload edges
function AgentActivityUI.show(toolName: string?)
	AgentActivityUI.setActiveTool(toolName)
end

function AgentActivityUI.hide()
	AgentActivityUI.setActiveTool(nil :: string?)
end

return AgentActivityUI
