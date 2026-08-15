local rpc = require("pi.rpc")
local context = require("pi.context")
local ui = require("pi.ui")

local M = {}

-- Module state
local client = nil
local ui_instance = nil
local active_req_id = nil

--- Initialize or retrieve shared RPC client and UI instance
---@param opts? table
---@return table client
---@return table ui_instance
function M.get_or_create_context(opts)
  opts = opts or {}
  if not client then
    client = rpc.new({
      bin = opts.bin or "pi-rs",
      model = opts.model or "opencode/deepseek-v4-flash-free",
    })

    -- Register global notification handlers
    client:on("pi/streamingChunk", function(params)
      if ui_instance and ui_instance.is_open and params.chunk then
        vim.schedule(function()
          ui_instance:append_stream_chunk(params.chunk)
        end)
      end
    end)

    client:on("pi/toolExecuting", function(params)
      if ui_instance and ui_instance.is_open and params.tool_name then
        vim.schedule(function()
          ui_instance:start_spinner("Executing tool: " .. params.tool_name)
          ui_instance:append_message("🔧 *Executing tool: " .. params.tool_name .. "*", "system")
        end)
      end
    end)

    client:on("pi/toolCompleted", function(params)
      if ui_instance and ui_instance.is_open then
        vim.schedule(function()
          local status_str = params.is_error and "❌ Tool failed" or "✓ Tool finished"
          ui_instance:start_spinner("Thinking...")
          ui_instance:append_message(status_str, "system")
        end)
      end
    end)
  end

  if not ui_instance then
    ui_instance = ui.new({
      model = opts.model or client.model,
      width = (opts.floating_window and opts.floating_window.width) or 0.82,
      height = (opts.floating_window and opts.floating_window.height) or 0.82,
      border = (opts.floating_window and opts.floating_window.border) or "rounded",
    })
  end

  return client, ui_instance
end

--- Get direct access to active RPC client
function M.get_client()
  return client
end

--- Toggle or open the floating chat UI
---@param opts? table
function M.chat(opts)
  local c, u = M.get_or_create_context(opts)

  u:toggle(
    function(text)
      -- On user prompt submitted in UI
      M.prompt(text, { from_ui = true })
    end,
    function()
      -- On user cancel
      M.stop()
    end
  )
end

--- Send a prompt with collected buffer context
---@param text string
---@param opts? table
function M.prompt(text, opts)
  opts = opts or {}
  local c, u = M.get_or_create_context(opts)

  if not text or text:match("^%s*$") then
    return
  end

  -- Format context if requested or from normal buffer
  local full_prompt = text
  if not opts.raw_prompt then
    full_prompt = context.format_context_prompt(text, {
      bufnr = opts.bufnr,
      selection = opts.selection,
      use_selection = opts.use_selection,
      include_buffer = opts.include_buffer,
    })
  end

  -- Open UI if not already open
  if not u.is_open and not opts.silent then
    u:open(
      function(submitted) M.prompt(submitted, { from_ui = true }) end,
      function() M.stop() end
    )
  end

  if u.is_open then
    u:append_message(text, "user")
    u:start_spinner("Thinking...")
    u:append_message("", "assistant") -- Initialize assistant message block for streaming
  end

  active_req_id = c:request("pi/prompt", { prompt = full_prompt }, function(err, result)
    active_req_id = nil
    if u.is_open then
      u:stop_spinner()
      if err then
        u:append_message("⚠️ Error: " .. tostring(err), "system")
      end
    end

    if opts.callback then
      opts.callback(err, result)
    end
  end)
end

--- Explain the selected code or current buffer
---@param opts? table
function M.explain(opts)
  opts = opts or {}
  local bufnr = opts.bufnr or vim.api.nvim_get_current_buf()
  local sel = context.get_visual_selection(bufnr)

  local prompt_text = "Please explain the following code in detail, highlighting key logic, invariants, and potential edge cases:"
  if not sel then
    prompt_text = "Please explain this entire file in detail, highlighting its architecture, key functions, and invariants:"
  end

  M.prompt(prompt_text, {
    bufnr = bufnr,
    selection = sel,
    include_buffer = (sel == nil),
  })
end

--- Fix diagnostic errors at cursor or in selection
---@param opts? table
function M.fix(opts)
  opts = opts or {}
  local bufnr = opts.bufnr or vim.api.nvim_get_current_buf()
  local sel = context.get_visual_selection(bufnr)
  local cursor = context.get_cursor_info()

  local diags
  if sel then
    diags = context.get_diagnostics(bufnr, sel.start_line, sel.end_line)
  else
    diags = context.get_diagnostics(bufnr, cursor.line, cursor.line)
    if #diags == 0 then
      -- Fallback to all buffer diagnostics
      diags = context.get_diagnostics(bufnr)
    end
  end

  if #diags == 0 then
    vim.notify("Pi: No LSP/compiler diagnostics found in the target range.", vim.log.levels.INFO)
    return
  end

  local diag_summary = context.format_diagnostics_summary(diags)
  local prompt_text = string.format(
    "Please fix the following compiler / linter diagnostics cleanly and surgically. Provide the exact corrected code:\n\n%s",
    diag_summary
  )

  M.prompt(prompt_text, {
    bufnr = bufnr,
    selection = sel,
    include_buffer = (sel == nil),
  })
end

--- Refactor current selection with user instructions
---@param opts? table
function M.refactor(opts)
  opts = opts or {}
  local bufnr = opts.bufnr or vim.api.nvim_get_current_buf()
  local sel = context.get_visual_selection(bufnr)

  if not sel then
    vim.notify("Pi: Please visually select a range of code to refactor.", vim.log.levels.WARN)
    return
  end

  vim.ui.input({ prompt = "🥧 Pi Refactor Instructions: " }, function(input)
    if not input or input:match("^%s*$") then
      return
    end

    local prompt_text = string.format(
      "Refactor the selected code according to these instructions: %s\nProvide the replacement code cleanly.",
      input
    )

    M.prompt(prompt_text, {
      bufnr = bufnr,
      selection = sel,
    })
  end)
end

--- Diff and apply replacement code to a buffer range
---@param new_text string Replacement text or markdown block
---@param start_line? number 1-indexed start line
---@param end_line? number 1-indexed end line
---@param bufnr? number Target buffer number
function M.diff_and_apply(new_text, start_line, end_line, bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local total_lines = vim.api.nvim_buf_line_count(bufnr)
  start_line = start_line or 1
  end_line = end_line or total_lines

  -- Extract clean code if fenced
  local clean_text = new_text
  local fenced = new_text:match("```[%w_-]*\n(.-)\n```")
  if fenced then
    clean_text = fenced
  end

  local original_lines = vim.api.nvim_buf_get_lines(bufnr, start_line - 1, end_line, false)
  local replacement_lines = vim.split(clean_text, "\n", { plain = true })

  ui.show_diff_preview(original_lines, replacement_lines, function()
    -- Apply patch to buffer
    vim.api.nvim_buf_set_lines(bufnr, start_line - 1, end_line, false, replacement_lines)
    vim.notify(
      string.format("✓ Pi: Applied changes to lines %d-%d.", start_line, start_line + #replacement_lines - 1),
      vim.log.levels.INFO
    )
  end, function()
    vim.notify("Pi: Rejected proposed changes.", vim.log.levels.INFO)
  end)
end

--- Query models from RPC and display interactive model selector
---@param opts? table
function M.models(opts)
  opts = opts or {}
  local c, u = M.get_or_create_context(opts)

  vim.notify("🥧 Pi: Querying available models across providers...", vim.log.levels.INFO)

  c:request("pi/models", { force = opts.force or false }, function(err, result)
    if err then
      vim.notify("Pi Error querying models: " .. tostring(err), vim.log.levels.ERROR)
      return
    end

    local models = (result and result.models) or {}
    if #models == 0 then
      vim.notify("Pi: No models returned from daemon.", vim.log.levels.WARN)
      return
    end

    local items = {}
    for _, m in ipairs(models) do
      local ctx = (m.context_window and m.context_window >= 1000000)
          and string.format("%dM", math.floor(m.context_window / 1000000))
        or string.format("%dk", math.floor((m.context_window or 128000) / 1000))

      local caps = {}
      if m.supports_reasoning then table.insert(caps, "Reasoning") end
      if m.supports_vision then table.insert(caps, "Vision") end
      local cap_str = #caps > 0 and table.concat(caps, "+") or "Standard"

      table.insert(items, {
        id = m.id,
        provider = m.provider,
        context = ctx,
        capabilities = cap_str,
        description = m.description or "",
        display = string.format("%-34s | %-12s | %-6s | %s", m.id, m.provider, ctx, cap_str),
      })
    end

    vim.ui.select(items, {
      prompt = "🥧 Select Pi Model:",
      format_item = function(item)
        return item.display
      end,
    }, function(choice)
      if choice then
        c:request("pi/model/set", { model = choice.id }, function(set_err, set_res)
          if set_err then
            vim.notify("Pi Error setting model: " .. tostring(set_err), vim.log.levels.ERROR)
          else
            c.model = choice.id
            u:set_model(choice.id)
            vim.notify(string.format("✓ Pi: Active model set to '%s' (%s)", choice.id, choice.provider), vim.log.levels.INFO)
          end
        end)
      end
    end)
  end)
end

--- Cancel active RPC turn
function M.stop()
  local c, u = M.get_or_create_context()
  if active_req_id and c then
    c:stop()
    active_req_id = nil
    c:start() -- restart fresh RPC daemon
  end

  if u and u.is_busy then
    u:stop_spinner()
    u:append_message("⏹️ Generation canceled.", "system")
  end
  vim.notify("Pi: Generation stopped.", vim.log.levels.INFO)
end

--- Replay session trajectory
---@param opts? table
function M.replay(opts)
  opts = opts or {}
  local c, u = M.get_or_create_context(opts)

  if not u.is_open then
    u:open()
  end

  u:append_message("🎬 Loading session trajectory...", "system")

  c:request("pi/session/trajectory", opts, function(err, result)
    if err then
      u:append_message("⚠️ Error loading trajectory: " .. tostring(err), "system")
      return
    end

    local traj = result and result.trajectory
    if not traj or not traj.steps then
      u:append_message("No trajectory steps found.", "system")
      return
    end

    u:append_message(
      string.format("=== Replaying Session: %s (%d steps, %d est. tokens) ===", traj.session_id, traj.total_steps, traj.total_estimated_tokens),
      "system"
    )

    for _, step in ipairs(traj.steps) do
      local role_map = {
        User = "user",
        Assistant = "assistant",
        System = "system",
        Tool = "tool",
      }
      local role = role_map[step.role] or "system"
      u:append_message(step.content, role)
    end
  end)
end

return M
