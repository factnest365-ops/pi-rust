local M = {}

---@class PiUi
---@field chat_buf number
---@field chat_win number
---@field input_buf number
---@field input_win number
---@field is_open boolean
---@field spinner_timer any
---@field spinner_idx number
---@field is_busy boolean
---@field status_text string
---@field active_model string
local PiUi = {}
PiUi.__index = PiUi

local SPINNER_FRAMES = { "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏" }

--- Create or return the PiUi singleton
---@param opts? table
---@return PiUi
function M.new(opts)
  opts = opts or {}
  local self = setmetatable({}, PiUi)
  self.chat_buf = nil
  self.chat_win = nil
  self.input_buf = nil
  self.input_win = nil
  self.is_open = false
  self.spinner_timer = nil
  self.spinner_idx = 1
  self.is_busy = false
  self.status_text = "Idle"
  self.active_model = opts.model or "opencode/deepseek-v4-flash-free"
  self.opts = opts
  return self
end

--- Calculate centered floating dimensions
---@param width_pct? number
---@param height_pct? number
---@return table Dimensions {width, height, row, col}
function M.calculate_dimensions(width_pct, height_pct)
  width_pct = width_pct or 0.82
  height_pct = height_pct or 0.82

  local total_cols = vim.o.columns
  local total_lines = vim.o.lines - vim.o.cmdheight - 1

  local width = math.max(40, math.floor(total_cols * width_pct))
  local height = math.max(12, math.floor(total_lines * height_pct))

  local row = math.max(0, math.floor((total_lines - height) / 2))
  local col = math.max(0, math.floor((total_cols - width) / 2))

  return {
    width = width,
    height = height,
    row = row,
    col = col,
  }
end

--- Open or toggle the floating chat UI
---@param on_submit fun(text: string)
---@param on_cancel fun()
function PiUi:open(on_submit, on_cancel)
  if self.is_open and self.chat_win and vim.api.nvim_win_is_valid(self.chat_win) then
    vim.api.nvim_set_current_win(self.input_win or self.chat_win)
    return
  end

  local dims = M.calculate_dimensions(self.opts.width, self.opts.height)
  local chat_height = math.max(6, dims.height - 4)
  local input_height = 3

  -- 1. Create or reuse chat transcript buffer
  if not self.chat_buf or not vim.api.nvim_buf_is_valid(self.chat_buf) then
    self.chat_buf = vim.api.nvim_create_buf(false, true)
    vim.bo[self.chat_buf].buftype = "nofile"
    vim.bo[self.chat_buf].bufhidden = "hide"
    vim.bo[self.chat_buf].swapfile = false
    vim.bo[self.chat_buf].filetype = "markdown"

    -- Initial welcome banner
    local banner = {
      "# 🥧 Pi Coding Agent (100% Pure Rust)",
      "",
      "Ready to assist. Press `i` or `<CR>` in the input box below to prompt.",
      "Keymaps: `<CR>` Submit | `<C-c>` Cancel | `<Tab>` Switch Pane | `q` / `<Esc>` Close",
      "─────────────────────────────────────────────────────────────────────────────",
      "",
    }
    vim.api.nvim_buf_set_lines(self.chat_buf, 0, -1, false, banner)
  end

  -- 2. Create chat window
  local chat_opts = {
    relative = "editor",
    width = dims.width,
    height = chat_height,
    row = dims.row,
    col = dims.col,
    style = "minimal",
    border = self.opts.border or "rounded",
    title = string.format(" 🥧 Pi Agent [%s] ", self.active_model),
    title_pos = "center",
  }
  self.chat_win = vim.api.nvim_open_win(self.chat_buf, true, chat_opts)
  vim.wo[self.chat_win].wrap = true
  vim.wo[self.chat_win].linebreak = true
  vim.wo[self.chat_win].cursorline = false
  vim.wo[self.chat_win].number = false
  vim.wo[self.chat_win].relativenumber = false

  -- 3. Create input buffer
  if not self.input_buf or not vim.api.nvim_buf_is_valid(self.input_buf) then
    self.input_buf = vim.api.nvim_create_buf(false, true)
    vim.bo[self.input_buf].buftype = "nofile"
    vim.bo[self.input_buf].bufhidden = "hide"
    vim.bo[self.input_buf].swapfile = false
  end

  -- 4. Create input window
  local input_opts = {
    relative = "editor",
    width = dims.width,
    height = input_height,
    row = dims.row + chat_height + 2,
    col = dims.col,
    style = "minimal",
    border = self.opts.border or "rounded",
    title = " Prompt (<CR> Submit | <C-c> Cancel) ",
    title_pos = "left",
  }
  self.input_win = vim.api.nvim_open_win(self.input_buf, true, input_opts)
  vim.wo[self.input_win].wrap = true
  vim.wo[self.input_win].linebreak = true

  self.is_open = true

  -- 5. Setup keymaps
  self:_setup_keymaps(on_submit, on_cancel)

  -- Focus input window and enter insert mode
  vim.api.nvim_set_current_win(self.input_win)
  vim.cmd("startinsert")
end

--- Setup buffer local keymaps
---@param on_submit fun(text: string)
---@param on_cancel fun()
function PiUi:_setup_keymaps(on_submit, on_cancel)
  local function close_all()
    self:close()
  end

  local function submit_input()
    if not self.input_buf or not vim.api.nvim_buf_is_valid(self.input_buf) then
      return
    end
    local lines = vim.api.nvim_buf_get_lines(self.input_buf, 0, -1, false)
    local text = vim.trim(table.concat(lines, "\n"))
    if text ~= "" then
      -- Clear input box
      vim.api.nvim_buf_set_lines(self.input_buf, 0, -1, false, { "" })
      if on_submit then
        on_submit(text)
      end
    end
  end

  local function cancel_action()
    if on_cancel then
      on_cancel()
    end
  end

  local function switch_pane()
    local cur_win = vim.api.nvim_get_current_win()
    if cur_win == self.input_win and self.chat_win and vim.api.nvim_win_is_valid(self.chat_win) then
      vim.api.nvim_set_current_win(self.chat_win)
    elseif cur_win == self.chat_win and self.input_win and vim.api.nvim_win_is_valid(self.input_win) then
      vim.api.nvim_set_current_win(self.input_win)
      vim.cmd("startinsert")
    end
  end

  local function clear_history()
    if self.chat_buf and vim.api.nvim_buf_is_valid(self.chat_buf) then
      vim.api.nvim_buf_set_lines(self.chat_buf, 0, -1, false, {
        "# 🥧 Pi Coding Agent",
        "",
        "History cleared.",
        "",
      })
    end
  end

  -- Input window keymaps
  local bopts = { buffer = self.input_buf, silent = true, noremap = true }
  vim.keymap.set({ "n", "i" }, "<CR>", submit_input, bopts)
  vim.keymap.set({ "n", "i" }, "<C-c>", cancel_action, bopts)
  vim.keymap.set("n", "q", close_all, bopts)
  vim.keymap.set("n", "<Esc>", close_all, bopts)
  vim.keymap.set({ "n", "i" }, "<Tab>", switch_pane, bopts)

  -- Chat window keymaps
  local copts = { buffer = self.chat_buf, silent = true, noremap = true }
  vim.keymap.set("n", "q", close_all, copts)
  vim.keymap.set("n", "<Esc>", close_all, copts)
  vim.keymap.set("n", "<C-c>", cancel_action, copts)
  vim.keymap.set("n", "<C-l>", clear_history, copts)
  vim.keymap.set("n", "<Tab>", switch_pane, copts)
  vim.keymap.set("n", "i", function()
    if self.input_win and vim.api.nvim_win_is_valid(self.input_win) then
      vim.api.nvim_set_current_win(self.input_win)
      vim.cmd("startinsert")
    end
  end, copts)
end

--- Close floating windows
function PiUi:close()
  if self.input_win and vim.api.nvim_win_is_valid(self.input_win) then
    pcall(vim.api.nvim_win_close, self.input_win, true)
    self.input_win = nil
  end
  if self.chat_win and vim.api.nvim_win_is_valid(self.chat_win) then
    pcall(vim.api.nvim_win_close, self.chat_win, true)
    self.chat_win = nil
  end
  self.is_open = false
  self:stop_spinner()
end

--- Toggle the UI
---@param on_submit fun(text: string)
---@param on_cancel fun()
function PiUi:toggle(on_submit, on_cancel)
  if self.is_open then
    self:close()
  else
    self:open(on_submit, on_cancel)
  end
end

--- Append content to chat transcript
---@param lines string|string[] Lines to append
---@param role? string "user" | "assistant" | "system" | "tool"
function PiUi:append_message(lines, role)
  if not self.chat_buf or not vim.api.nvim_buf_is_valid(self.chat_buf) then
    return
  end

  if type(lines) == "string" then
    lines = vim.split(lines, "\n", { plain = true })
  end

  local prefix = ""
  if role == "user" then
    prefix = "### 👤 User:\n"
  elseif role == "assistant" then
    prefix = "### 🥧 Pi (" .. self.active_model .. "):\n"
  elseif role == "tool" then
    prefix = "#### 🔧 Tool Result:\n"
  elseif role == "system" then
    prefix = "#### ℹ️ System:\n"
  end

  if prefix ~= "" then
    local prefix_lines = vim.split(prefix, "\n", { plain = true })
    for _, l in ipairs(lines) do
      table.insert(prefix_lines, l)
    end
    lines = prefix_lines
  end

  table.insert(lines, "") -- empty separator line

  local cur_count = vim.api.nvim_buf_line_count(self.chat_buf)
  vim.api.nvim_buf_set_lines(self.chat_buf, cur_count, -1, false, lines)

  self:scroll_to_bottom()
end

--- Stream a chunk of text into the active assistant turn
---@param chunk string
function PiUi:append_stream_chunk(chunk)
  if not self.chat_buf or not vim.api.nvim_buf_is_valid(self.chat_buf) then
    return
  end

  local cur_count = vim.api.nvim_buf_line_count(self.chat_buf)
  local last_line = vim.api.nvim_buf_get_lines(self.chat_buf, cur_count - 1, cur_count, false)[1] or ""

  local chunk_lines = vim.split(chunk, "\n", { plain = true })
  if #chunk_lines == 1 then
    vim.api.nvim_buf_set_lines(self.chat_buf, cur_count - 1, cur_count, false, { last_line .. chunk_lines[1] })
  else
    local new_lines = { last_line .. chunk_lines[1] }
    for i = 2, #chunk_lines do
      table.insert(new_lines, chunk_lines[i])
    end
    vim.api.nvim_buf_set_lines(self.chat_buf, cur_count - 1, cur_count, false, new_lines)
  end

  self:scroll_to_bottom()
end

--- Scroll chat transcript window to bottom
function PiUi:scroll_to_bottom()
  if self.chat_win and vim.api.nvim_win_is_valid(self.chat_win) and self.chat_buf and vim.api.nvim_buf_is_valid(self.chat_buf) then
    local line_count = vim.api.nvim_buf_line_count(self.chat_buf)
    pcall(vim.api.nvim_win_set_cursor, self.chat_win, { line_count, 0 })
  end
end

--- Start spinner animation in window title
---@param status_text? string
function PiUi:start_spinner(status_text)
  self.is_busy = true
  self.status_text = status_text or "Thinking..."
  self:stop_spinner()

  local uv = vim.uv or vim.loop
  self.spinner_timer = uv.new_timer()
  self.spinner_timer:start(80, 80, vim.schedule_wrap(function()
    if not self.is_busy or not self.is_open or not self.chat_win or not vim.api.nvim_win_is_valid(self.chat_win) then
      return
    end
    self.spinner_idx = (self.spinner_idx % #SPINNER_FRAMES) + 1
    local frame = SPINNER_FRAMES[self.spinner_idx]
    local title = string.format(" %s %s [%s] ", frame, self.status_text, self.active_model)
    pcall(vim.api.nvim_win_set_config, self.chat_win, { title = title, title_pos = "center" })
  end))
end

--- Stop spinner animation
function PiUi:stop_spinner()
  self.is_busy = false
  if self.spinner_timer then
    pcall(function() self.spinner_timer:close() end)
    self.spinner_timer = nil
  end
  if self.chat_win and vim.api.nvim_win_is_valid(self.chat_win) then
    local title = string.format(" 🥧 Pi Agent [%s] ", self.active_model)
    pcall(vim.api.nvim_win_set_config, self.chat_win, { title = title, title_pos = "center" })
  end
end

--- Set active model in UI badge
---@param model_id string
function PiUi:set_model(model_id)
  self.active_model = model_id
  if self.chat_win and vim.api.nvim_win_is_valid(self.chat_win) then
    local title = string.format(" 🥧 Pi Agent [%s] ", self.active_model)
    pcall(vim.api.nvim_win_set_config, self.chat_win, { title = title, title_pos = "center" })
  end
end

--- Show an interactive Diff Preview popup allowing user to Accept (y) or Reject (n/q)
---@param original_lines string[]
---@param replacement_lines string[]
---@param on_accept fun()
---@param on_reject? fun()
function M.show_diff_preview(original_lines, replacement_lines, on_accept, on_reject)
  local dims = M.calculate_dimensions(0.85, 0.8)

  local buf = vim.api.nvim_create_buf(false, true)
  vim.bo[buf].buftype = "nofile"
  vim.bo[buf].filetype = "diff"
  vim.bo[buf].swapfile = false

  -- Build unified diff lines
  local diff_lines = {
    "--- Original",
    "+++ Proposed Replacement",
    "@@ Pending Review (Press [y] to Apply, [n] or [q] to Reject) @@",
  }

  for _, line in ipairs(original_lines) do
    table.insert(diff_lines, "-" .. line)
  end
  for _, line in ipairs(replacement_lines) do
    table.insert(diff_lines, "+" .. line)
  end

  vim.api.nvim_buf_set_lines(buf, 0, -1, false, diff_lines)

  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    width = dims.width,
    height = dims.height,
    row = dims.row,
    col = dims.col,
    style = "minimal",
    border = "rounded",
    title = " 🥧 Pi Code Diff Preview ([y] Apply | [n]/[q] Reject) ",
    title_pos = "center",
  })

  local function close_diff()
    if win and vim.api.nvim_win_is_valid(win) then
      pcall(vim.api.nvim_win_close, win, true)
    end
  end

  local opts = { buffer = buf, silent = true, noremap = true }
  vim.keymap.set("n", "y", function()
    close_diff()
    if on_accept then
      on_accept()
    end
  end, opts)

  vim.keymap.set("n", "<CR>", function()
    close_diff()
    if on_accept then
      on_accept()
    end
  end, opts)

  vim.keymap.set("n", "n", function()
    close_diff()
    if on_reject then
      on_reject()
    end
  end, opts)

  vim.keymap.set("n", "q", function()
    close_diff()
    if on_reject then
      on_reject()
    end
  end, opts)

  vim.keymap.set("n", "<Esc>", function()
    close_diff()
    if on_reject then
      on_reject()
    end
  end, opts)
end

return M
