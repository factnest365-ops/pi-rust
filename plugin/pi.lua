if vim.g.loaded_pi_nvim == 1 then
  return
end
vim.g.loaded_pi_nvim = 1

local pi = require("pi")

-- Define :Pi user command
vim.api.nvim_create_user_command("Pi", function(opts)
  local args = vim.trim(opts.args or "")
  local bufnr = vim.api.nvim_get_current_buf()
  local range_opts = {}

  if opts.range > 0 then
    local start_line = opts.line1
    local end_line = opts.line2
    local lines = vim.api.nvim_buf_get_lines(bufnr, start_line - 1, end_line, false)
    range_opts.selection = {
      start_line = start_line,
      start_col = 1,
      end_line = end_line,
      end_col = string.len(lines[#lines] or "") + 1,
      lines = lines,
      text = table.concat(lines, "\n"),
      line_count = #lines,
    }
  end

  if args == "" then
    pi.chat(range_opts)
  else
    pi.prompt(args, range_opts)
  end
end, {
  nargs = "*",
  range = true,
  desc = "🥧 Pi Coding Agent: Prompt or Chat",
})

-- Define :PiChat user command
vim.api.nvim_create_user_command("PiChat", function()
  pi.chat()
end, {
  desc = "🥧 Pi Coding Agent: Toggle Chat UI",
})

-- Define :PiExplain user command
vim.api.nvim_create_user_command("PiExplain", function(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local range_opts = { bufnr = bufnr }

  if opts.range > 0 then
    local start_line = opts.line1
    local end_line = opts.line2
    local lines = vim.api.nvim_buf_get_lines(bufnr, start_line - 1, end_line, false)
    range_opts.selection = {
      start_line = start_line,
      start_col = 1,
      end_line = end_line,
      end_col = string.len(lines[#lines] or "") + 1,
      lines = lines,
      text = table.concat(lines, "\n"),
      line_count = #lines,
    }
  end

  pi.explain(range_opts)
end, {
  range = true,
  desc = "🥧 Pi Coding Agent: Explain Selection or Buffer",
})

-- Define :PiFix user command
vim.api.nvim_create_user_command("PiFix", function(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local range_opts = { bufnr = bufnr }

  if opts.range > 0 then
    local start_line = opts.line1
    local end_line = opts.line2
    local lines = vim.api.nvim_buf_get_lines(bufnr, start_line - 1, end_line, false)
    range_opts.selection = {
      start_line = start_line,
      start_col = 1,
      end_line = end_line,
      end_col = string.len(lines[#lines] or "") + 1,
      lines = lines,
      text = table.concat(lines, "\n"),
      line_count = #lines,
    }
  end

  pi.fix(range_opts)
end, {
  range = true,
  desc = "🥧 Pi Coding Agent: Fix Diagnostic Errors",
})

-- Define :PiRefactor user command
vim.api.nvim_create_user_command("PiRefactor", function(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local range_opts = { bufnr = bufnr }

  if opts.range > 0 then
    local start_line = opts.line1
    local end_line = opts.line2
    local lines = vim.api.nvim_buf_get_lines(bufnr, start_line - 1, end_line, false)
    range_opts.selection = {
      start_line = start_line,
      start_col = 1,
      end_line = end_line,
      end_col = string.len(lines[#lines] or "") + 1,
      lines = lines,
      text = table.concat(lines, "\n"),
      line_count = #lines,
    }
  end

  local instructions = vim.trim(opts.args or "")
  if instructions ~= "" then
    local prompt_text = string.format(
      "Refactor the selected code according to these instructions: %s\nProvide the replacement code cleanly.",
      instructions
    )
    pi.prompt(prompt_text, range_opts)
  else
    pi.refactor(range_opts)
  end
end, {
  nargs = "*",
  range = true,
  desc = "🥧 Pi Coding Agent: Refactor Selected Code",
})

-- Define :PiModels user command
vim.api.nvim_create_user_command("PiModels", function(opts)
  local force = opts.bang or (opts.args == "refresh")
  pi.models({ force = force })
end, {
  bang = true,
  nargs = "?",
  desc = "🥧 Pi Coding Agent: Select Active Model",
})

-- Define :PiStop user command
vim.api.nvim_create_user_command("PiStop", function()
  pi.stop()
end, {
  desc = "🥧 Pi Coding Agent: Stop Active Generation",
})

-- Define :PiReplay user command
vim.api.nvim_create_user_command("PiReplay", function(opts)
  local branch_id = vim.trim(opts.args or "")
  pi.replay({ branch_node_id = branch_id ~= "" and branch_id or nil })
end, {
  nargs = "?",
  desc = "🥧 Pi Coding Agent: Replay Session Trajectory",
})
