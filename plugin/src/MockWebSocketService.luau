local HttpService = game:GetService("HttpService")

local MockWebSocketClient = {}
MockWebSocketClient.__index = MockWebSocketClient

local EnumWebSocketState = {
	Connecting = "Connecting",
	Open = "Open",
	Closing = "Closing",
	Closed = "Closed",
}

local CloseableStates = {
	[EnumWebSocketState.Connecting] = true,
	[EnumWebSocketState.Open] = true,
}

local POLL_WAIT_TIME = 1

export type MockWebSocketClient = {
	Send: (self: MockWebSocketClient, data: any) -> (),
	Close: (self: MockWebSocketClient) -> (),
	SetReceiveEndpoint: (self: MockWebSocketClient, endpoint: string) -> (),
	SetSendEndpoint: (self: MockWebSocketClient, endpoint: string) -> (),
	Opened: RBXScriptSignal,
	Closed: RBXScriptSignal,
	MessageReceived: RBXScriptSignal,
	AgentActivityChanged: RBXScriptSignal,
	ConnectionState: typeof(EnumWebSocketState.Connecting),
}

type MockWebSocketClientPrivate = MockWebSocketClient & {
	new: (uri: string) -> MockWebSocketClient,
	_OpenImpl: (self: MockWebSocketClient) -> (),
	_uri: string,
	_receiveEndpoint: string,
	_sendEndpoint: string,
	_pollTask: thread?,
	_OpenedEvent: BindableEvent,
	_ClosedEvent: BindableEvent,
	_MessageReceivedEvent: BindableEvent,
	_AgentActivityChangedEvent: BindableEvent,
}

function MockWebSocketClient.new(uri: string): MockWebSocketClient
	local self: MockWebSocketClientPrivate = setmetatable({}, MockWebSocketClient) :: any

	self._uri = uri
	self._receiveEndpoint = ""
	self._sendEndpoint = ""
	self._pollTask = nil :: thread?

	self._OpenedEvent = Instance.new("BindableEvent")
	self.Opened = self._OpenedEvent.Event

	self._ClosedEvent = Instance.new("BindableEvent")
	self.Closed = self._ClosedEvent.Event

	self._MessageReceivedEvent = Instance.new("BindableEvent")
	self.MessageReceived = self._MessageReceivedEvent.Event

	self._AgentActivityChangedEvent = Instance.new("BindableEvent")
	self.AgentActivityChanged = self._AgentActivityChangedEvent.Event

	self.ConnectionState = EnumWebSocketState.Connecting

	task.defer(self._OpenImpl, self)

	return self
end

local function doRequest(url: string, method: "GET" | "POST", body: any)
	local ok, response = pcall(function()
		return HttpService:RequestAsync({
			Url = url,
			Method = method,
			Headers = {
				["Content-Type"] = "application/json",
			},
			Body = if body then HttpService:JSONEncode(body) else nil,
			Compress = Enum.HttpCompression.None,
		})
	end)

	return if ok then response else nil
end

function MockWebSocketClient._OpenImpl(self: MockWebSocketClientPrivate)
	assert(self.ConnectionState == EnumWebSocketState.Connecting, "WebSocket is not in the Connecting state")

	self.ConnectionState = EnumWebSocketState.Open
	self._OpenedEvent:Fire()

	self._pollTask = task.spawn(function()
		local lastPollTick = 0
		local lastAgentStatus = false
		local lastAgentTool: string? = nil
		
		while self.ConnectionState == EnumWebSocketState.Open do
			if tick() - lastPollTick >= POLL_WAIT_TIME then
				-- Poll main message queue
				local messageResponse = doRequest(self._uri .. self._receiveEndpoint, "GET")
				if messageResponse and messageResponse.StatusCode == 200 and messageResponse.Success then
					self._MessageReceivedEvent:Fire(messageResponse.Body)
				end
				
				-- Poll agent activity
				local agentResponse = doRequest(self._uri .. "/agent_status", "GET")
				if agentResponse and agentResponse.Success and agentResponse.StatusCode == 200 then
					local body = HttpService:JSONDecode(agentResponse.Body :: string)
					if body and type(body.is_working) == "boolean" then
						local currentAgentStatus = body.is_working
						local currentTool = body.current_tool  -- may be nil
						if currentAgentStatus ~= lastAgentStatus or currentTool ~= lastAgentTool then
							self._AgentActivityChangedEvent:Fire(currentAgentStatus, currentTool)
							lastAgentStatus = currentAgentStatus
							lastAgentTool = currentTool
						end
					end
				end
				
				lastPollTick = tick()
			end
			
			task.wait(0.2)
		end
	end)
end

function MockWebSocketClient.Send(self: MockWebSocketClientPrivate, data: any)
	doRequest(self._uri .. self._sendEndpoint, "POST", data)
end

function MockWebSocketClient.Close(self: MockWebSocketClientPrivate)
	if CloseableStates[self.ConnectionState] then
		self.ConnectionState = EnumWebSocketState.Closing

		if self._pollTask and coroutine.status(self._pollTask) == "running" then
			task.cancel(self._pollTask)
			self._pollTask = nil :: any
		end

		self.ConnectionState = EnumWebSocketState.Closed
		self._ClosedEvent:Fire()
	end
end

-- START DEVIATION: These methods are not present in the real WebSocketClient instance
function MockWebSocketClient.SetReceiveEndpoint(self: MockWebSocketClientPrivate, endpoint: string)
	self._receiveEndpoint = endpoint
end

function MockWebSocketClient.SetSendEndpoint(self: MockWebSocketClientPrivate, endpoint: string)
	self._sendEndpoint = endpoint
end
-- END DEVIATION

local MockWebSocketService = {}
MockWebSocketService.__index = MockWebSocketService

type MockWebSocketService = {
	CreateClient: (self: MockWebSocketService, uri: string) -> MockWebSocketClient,
}

function MockWebSocketService.CreateClient(_: MockWebSocketService, uri: string)
	return MockWebSocketClient.new(uri)
end

return MockWebSocketService
