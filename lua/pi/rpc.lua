local M = {}

---@class RpcClient
---@field bin string Executable path for pi-rs
---@field model string Default model ID
---@field job_id? number Neovim job ID
---@field req_id number Monotonic request ID counter
---@field pending table<number, {resolve: function, reject: function, method: string, timer?: any}>
---@field listeners table<string, function[]> Map of event notifications to callbacks
---@field buffer string Incomplete line buffer for stdout
---@field is_active boolean Whether the RPC job is currently active
local RpcClient = {}
RpcClient.__index = RpcClient

--- Create a new JSON-RPC 2.0 client instance
---@param opts? {bin?: string, model?: string}
---@return RpcClient
function M.new(opts)
  opts = opts or {}
  local self = setmetatable({}, RpcClient)
  self.bin = opts.bin or "pi-rs"
  self.model = opts.model or "opencode/deepseek-v4-flash-free"
  self.job_id = nil
  self.req_id = 0
  self.pending = {}
  self.listeners = {}
  self.buffer = ""
  self.is_active = false
  return self
end

--- Find pi-rs executable on PATH or in standard target directories
---@param bin_name? string
---@return string? Absolute or relative path to executable
function M.find_binary(bin_name)
  bin_name = bin_name or "pi-rs"
  -- Check if direct path or on PATH
  if vim.fn.executable(bin_name) == 1 then
    return bin_name
  end

  -- Check common cargo build target directories relative to current working directory
  local candidates = {
    "target/release/" .. bin_name,
    "target/debug/" .. bin_name,
    vim.fn.expand("~/.cargo/bin/" .. bin_name),
    vim.fn.expand("~/.local/bin/" .. bin_name),
    "/usr/local/bin/" .. bin_name,
    "/opt/homebrew/bin/" .. bin_name,
  }

  for _, candidate in ipairs(candidates) do
    if vim.fn.executable(candidate) == 1 then
      return candidate
    end
  end

  return nil
end

--- Start the RPC daemon process if not already running
---@return boolean Success status
---@return string? Error message if failed
function RpcClient:start()
  if self.is_active and self.job_id then
    return true, nil
  end

  local bin_path = M.find_binary(self.bin)
  if not bin_path then
    local err_msg = string.format("Pi binary '%s' not found in PATH or standard cargo target paths.", self.bin)
    return false, err_msg
  end

  self.buffer = ""
  local cmd = { bin_path, "--rpc" }
  if self.model and self.model ~= "" then
    table.insert(cmd, "--model")
    table.insert(cmd, self.model)
  end

  local job_id = vim.fn.jobstart(cmd, {
    on_stdout = function(_, data, _)
      self:_handle_stdout(data)
    end,
    on_stderr = function(_, data, _)
      self:_handle_stderr(data)
    end,
    on_exit = function(_, exit_code, _)
      self:_handle_exit(exit_code)
    end,
    stdout_buffered = false,
    stderr_buffered = false,
  })

  if job_id <= 0 then
    local err_msg = string.format("Failed to spawn pi-rs RPC job (code: %d)", job_id)
    self.is_active = false
    self.job_id = nil
    return false, err_msg
  end

  self.job_id = job_id
  self.is_active = true
  return true, nil
end

--- Stop the RPC daemon process
function RpcClient:stop()
  if self.job_id then
    -- Reject all pending requests
    for req_id, p in pairs(self.pending) do
      if p.reject then
        p.reject("RPC client terminated")
      end
      if p.timer then
        pcall(function() p.timer:close() end)
      end
      self.pending[req_id] = nil
    end

    pcall(function()
      vim.fn.jobstop(self.job_id)
    end)
    self.job_id = nil
    self.is_active = false
  end
end

--- Check if the client process is alive
---@return boolean
function RpcClient:is_running()
  return self.is_active and self.job_id ~= nil
end

--- Internal handler for raw stdout chunks from jobstart
---@param chunks string[]
function RpcClient:_handle_stdout(chunks)
  if not chunks or #chunks == 0 then
    return
  end

  -- In Neovim jobstart, chunks is a table of lines where last element is unfinished line
  for i, chunk in ipairs(chunks) do
    if i == 1 then
      self.buffer = self.buffer .. chunk
    else
      -- Previous line completed, parse it
      local complete_line = self.buffer
      self.buffer = chunk
      self:_parse_line(complete_line)
    end
  end
end

--- Internal handler for stderr
---@param chunks string[]
function RpcClient:_handle_stderr(chunks)
  if not chunks or #chunks == 0 then
    return
  end
  -- In --rpc mode, operational and debug logs from pi-rs arrive on stderr
  -- We don't interfere unless requested
end

--- Internal handler for job exit
---@param exit_code number
function RpcClient:_handle_exit(exit_code)
  self.is_active = false
  self.job_id = nil

  -- Flush any remaining buffer
  if self.buffer and #self.buffer > 0 then
    self:_parse_line(self.buffer)
    self.buffer = ""
  end

  -- Reject any pending callbacks
  for req_id, p in pairs(self.pending) do
    if p.reject then
      p.reject(string.format("RPC daemon exited unexpectedly with code %d", exit_code))
    end
    if p.timer then
      pcall(function() p.timer:close() end)
    end
    self.pending[req_id] = nil
  end

  self:_emit("exit", { exit_code = exit_code })
end

--- Parse a single line as JSON-RPC frame
---@param line string
function RpcClient:_parse_line(line)
  line = vim.trim(line)
  if line == "" then
    return
  end

  local ok, frame = pcall(vim.json.decode, line)
  if not ok or type(frame) ~= "table" then
    return
  end

  -- Check if response (has 'id')
  if frame.id ~= nil and frame.id ~= vim.NIL then
    local req_id = frame.id
    local pending = self.pending[req_id]
    if pending then
      self.pending[req_id] = nil
      if pending.timer then
        pcall(function() pending.timer:close() end)
      end

      if frame.error then
        if pending.reject then
          local err_msg = frame.error.message or "Unknown RPC error"
          pending.reject(err_msg, frame.error)
        end
      else
        if pending.resolve then
          pending.resolve(frame.result)
        end
      end
    end
    return
  end

  -- Check if notification (has 'method', no 'id')
  if frame.method and type(frame.method) == "string" then
    self:_emit(frame.method, frame.params or {})
  end
end

--- Send a JSON-RPC 2.0 request and register a callback
---@param method string RPC method name (e.g. "pi/prompt", "pi/models")
---@param params? table Method parameters
---@param callback? fun(err: string?, result: any?) Callback function
---@param timeout_ms? number Timeout in milliseconds (default: 180000 / 3m)
---@return number? Request ID
function RpcClient:request(method, params, callback, timeout_ms)
  if not self:is_running() then
    local ok, err = self:start()
    if not ok then
      if callback then
        vim.schedule(function() callback(err, nil) end)
      end
      return nil
    end
  end

  self.req_id = self.req_id + 1
  local req_id = self.req_id

  local payload = {
    jsonrpc = "2.0",
    id = req_id,
    method = method,
    params = params or {},
  }

  local ok, encoded = pcall(vim.json.encode, payload)
  if not ok then
    if callback then
      vim.schedule(function() callback("Failed to JSON-encode request", nil) end)
    end
    return nil
  end

  local timer = nil
  timeout_ms = timeout_ms or 180000
  if timeout_ms > 0 then
    local uv = vim.uv or vim.loop
    timer = uv.new_timer()
    timer:start(timeout_ms, 0, vim.schedule_wrap(function()
      if self.pending[req_id] then
        local p = self.pending[req_id]
        self.pending[req_id] = nil
        if p.timer then
          pcall(function() p.timer:close() end)
        end
        if p.reject then
          p.reject("RPC request timed out after " .. tostring(timeout_ms) .. "ms")
        end
      end
    end))
  end

  self.pending[req_id] = {
    method = method,
    timer = timer,
    resolve = function(res)
      if callback then
        vim.schedule(function() callback(nil, res) end)
      end
    end,
    reject = function(err, data)
      if callback then
        vim.schedule(function() callback(err or "RPC error", nil) end)
      end
    end,
  }

  pcall(function()
    vim.fn.chansend(self.job_id, encoded .. "\n")
  end)

  return req_id
end

--- Send a synchronous JSON-RPC request (blocking with vim.wait)
---@param method string
---@param params? table
---@param timeout_ms? number
---@return string? error
---@return any? result
function RpcClient:request_sync(method, params, timeout_ms)
  timeout_ms = timeout_ms or 10000
  local done = false
  local res_err = nil
  local res_val = nil

  self:request(method, params, function(err, res)
    res_err = err
    res_val = res
    done = true
  end, timeout_ms)

  vim.wait(timeout_ms, function()
    return done
  end, 20)

  if not done then
    return "Request timed out", nil
  end

  return res_err, res_val
end

--- Send a JSON-RPC notification (no response expected)
---@param method string
---@param params? table
function RpcClient:notify(method, params)
  if not self:is_running() then
    local ok = self:start()
    if not ok then return end
  end

  local payload = {
    jsonrpc = "2.0",
    method = method,
    params = params or {},
  }

  local ok, encoded = pcall(vim.json.encode, payload)
  if ok and self.job_id then
    pcall(function()
      vim.fn.chansend(self.job_id, encoded .. "\n")
    end)
  end
end

--- Subscribe to a JSON-RPC notification event
---@param event string Event name (e.g. "pi/streamingChunk", "pi/toolExecuting")
---@param handler fun(params: table)
function RpcClient:on(event, handler)
  if not self.listeners[event] then
    self.listeners[event] = {}
  end
  table.insert(self.listeners[event], handler)
end

--- Unsubscribe from an event
---@param event string
---@param handler fun(params: table)
function RpcClient:off(event, handler)
  if not self.listeners[event] then
    return
  end
  for i, h in ipairs(self.listeners[event]) do
    if h == handler then
      table.remove(self.listeners[event], i)
      break
    end
  end
end

--- Emit an event to all registered listeners
---@param event string
---@param params table
function RpcClient:_emit(event, params)
  local handlers = self.listeners[event]
  if handlers then
    for _, handler in ipairs(handlers) do
      pcall(handler, params)
    end
  end
end

return M
