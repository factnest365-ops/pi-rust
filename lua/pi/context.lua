local M = {}

--- Get active buffer information
---@param bufnr? number Buffer number (defaults to current buffer)
---@return table Buffer metadata
function M.get_active_buffer_info(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local name = vim.api.nvim_buf_get_name(bufnr)
  local filetype = vim.bo[bufnr].filetype or ""
  local total_lines = vim.api.nvim_buf_line_count(bufnr)
  local is_modified = vim.bo[bufnr].modified
  local is_readonly = vim.bo[bufnr].readonly or not vim.bo[bufnr].modifiable
  local cwd = vim.fn.getcwd()
  
  local relative_path = name
  if name ~= "" and vim.startswith(name, cwd) then
    relative_path = name:sub(#cwd + 2)
  elseif name == "" then
    relative_path = "[No Name]"
  end

  return {
    bufnr = bufnr,
    name = name,
    relative_path = relative_path,
    filetype = filetype,
    total_lines = total_lines,
    is_modified = is_modified,
    is_readonly = is_readonly,
    cwd = cwd,
  }
end

--- Get cursor position and current line text
---@param winnr? number Window number (defaults to current window)
---@return table Cursor metadata
function M.get_cursor_info(winnr)
  winnr = winnr or vim.api.nvim_get_current_win()
  local cursor = vim.api.nvim_win_get_cursor(winnr)
  local line = cursor[1]
  local col = cursor[2] + 1
  local bufnr = vim.api.nvim_win_get_buf(winnr)
  local lines = vim.api.nvim_buf_get_lines(bufnr, line - 1, line, false)
  local line_text = lines[1] or ""

  return {
    line = line,
    col = col,
    line_text = line_text,
  }
end

--- Get visual selection range and text
---@param bufnr? number Buffer number
---@return table? Selection range and content, or nil if no selection
function M.get_visual_selection(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local mode = vim.fn.mode()
  local is_visual = mode == "v" or mode == "V" or mode == "\22"

  local start_pos, end_pos

  if is_visual then
    -- Currently in visual mode
    start_pos = vim.fn.getpos("v")
    end_pos = vim.fn.getpos(".")
  else
    -- Use last visual selection marks '< and '>
    start_pos = vim.fn.getpos("'<")
    end_pos = vim.fn.getpos("'>")
  end

  local start_line = start_pos[2]
  local start_col = start_pos[3]
  local end_line = end_pos[2]
  local end_col = end_pos[3]

  -- If marks are not set or buffer has changed
  if start_line == 0 or end_line == 0 then
    return nil
  end

  -- Swap if backwards selection
  if start_line > end_line or (start_line == end_line and start_col > end_col) then
    start_line, end_line = end_line, start_line
    start_col, end_col = end_col, start_col
  end

  local total_lines = vim.api.nvim_buf_line_count(bufnr)
  if start_line > total_lines or end_line > total_lines then
    return nil
  end

  local lines = vim.api.nvim_buf_get_lines(bufnr, start_line - 1, end_line, false)
  if #lines == 0 then
    return nil
  end

  local selected_text
  if mode == "V" or (not is_visual and start_col == 1 and end_col == 2147483647) then
    -- Line-wise visual
    selected_text = table.concat(lines, "\n")
  else
    -- Character-wise visual
    if #lines == 1 then
      lines[1] = string.sub(lines[1], start_col, end_col)
    else
      lines[1] = string.sub(lines[1], start_col)
      lines[#lines] = string.sub(lines[#lines], 1, end_col)
    end
    selected_text = table.concat(lines, "\n")
  end

  return {
    start_line = start_line,
    start_col = start_col,
    end_line = end_line,
    end_col = end_col,
    lines = lines,
    text = selected_text,
    line_count = #lines,
  }
end

--- Extract active LSP diagnostics for buffer and optional line range
---@param bufnr? number
---@param start_line? number 1-indexed start line
---@param end_line? number 1-indexed end line
---@return table[] Array of structured diagnostics
function M.get_diagnostics(bufnr, start_line, end_line)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  if not vim.diagnostic or not vim.diagnostic.get then
    return {}
  end

  local diags = vim.diagnostic.get(bufnr)
  local severity_names = {
    [vim.diagnostic.severity.ERROR] = "ERROR",
    [vim.diagnostic.severity.WARN] = "WARN",
    [vim.diagnostic.severity.INFO] = "INFO",
    [vim.diagnostic.severity.HINT] = "HINT",
  }

  local results = {}
  for _, d in ipairs(diags) do
    local lnum = d.lnum + 1 -- convert to 1-indexed
    local in_range = true
    if start_line and end_line then
      in_range = (lnum >= start_line and lnum <= end_line)
    elseif start_line then
      in_range = (lnum == start_line)
    end

    if in_range then
      table.insert(results, {
        line = lnum,
        col = (d.col or 0) + 1,
        severity = severity_names[d.severity] or "INFO",
        severity_num = d.severity,
        message = d.message,
        source = d.source or "lsp",
        code = d.code,
      })
    end
  end

  -- Sort by line and severity
  table.sort(results, function(a, b)
    if a.line == b.line then
      return a.severity_num < b.severity_num
    end
    return a.line < b.line
  end)

  return results
end

--- Format diagnostics into a clean markdown summary
---@param diags table[]
---@return string
function M.format_diagnostics_summary(diags)
  if not diags or #diags == 0 then
    return ""
  end

  local out = { "### LSP / Compiler Diagnostics:" }
  for _, d in ipairs(diags) do
    local code_str = d.code and (" [" .. tostring(d.code) .. "]") or ""
    table.insert(
      out,
      string.format("- Line %d:%d [%s] (%s%s): %s", d.line, d.col, d.severity, d.source, code_str, d.message)
    )
  end
  return table.concat(out, "\n")
end

--- Build a structured prompt injecting buffer, selection, and diagnostics context
---@param user_prompt string
---@param opts? table Additional options (e.g. bufnr, selection, range)
---@return string Formatted full prompt
function M.format_context_prompt(user_prompt, opts)
  opts = opts or {}
  local bufnr = opts.bufnr or vim.api.nvim_get_current_buf()
  local buf_info = M.get_active_buffer_info(bufnr)
  local selection = opts.selection or (opts.use_selection and M.get_visual_selection(bufnr))
  local cursor = M.get_cursor_info()

  local parts = {}

  -- Add user prompt
  if user_prompt and user_prompt:match("%S") then
    table.insert(parts, user_prompt)
    table.insert(parts, "")
  end

  -- Add file context
  if buf_info.name ~= "" then
    table.insert(parts, string.format("--- File Context: `%s` (%s) ---", buf_info.relative_path, buf_info.filetype))
    
    if selection then
      table.insert(
        parts,
        string.format(
          "Selected Lines %d-%d of %d in `%s`:",
          selection.start_line,
          selection.end_line,
          buf_info.total_lines,
          buf_info.relative_path
        )
      )
      table.insert(parts, string.format("```%s", buf_info.filetype))
      table.insert(parts, selection.text)
      table.insert(parts, "```")

      -- Selection diagnostics
      local diags = M.get_diagnostics(bufnr, selection.start_line, selection.end_line)
      if #diags > 0 then
        table.insert(parts, "")
        table.insert(parts, M.format_diagnostics_summary(diags))
      end
    elseif opts.include_buffer then
      -- Full buffer context
      local all_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
      table.insert(parts, string.format("Full Buffer Content (`%s`):", buf_info.relative_path))
      table.insert(parts, string.format("```%s", buf_info.filetype))
      table.insert(parts, table.concat(all_lines, "\n"))
      table.insert(parts, "```")

      -- All buffer diagnostics
      local diags = M.get_diagnostics(bufnr)
      if #diags > 0 then
        table.insert(parts, "")
        table.insert(parts, M.format_diagnostics_summary(diags))
      end
    else
      -- Cursor context
      table.insert(
        parts,
        string.format("Cursor at Line %d, Column %d in `%s`", cursor.line, cursor.col, buf_info.relative_path)
      )
      
      -- Diagnostics at or near cursor line
      local diags = M.get_diagnostics(bufnr, cursor.line, cursor.line)
      if #diags > 0 then
        table.insert(parts, "")
        table.insert(parts, M.format_diagnostics_summary(diags))
      end
    end
  end

  return table.concat(parts, "\n")
end

return M
