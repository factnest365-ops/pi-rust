if vim.g.loaded_tau_nvim == 1 then
  return
end
vim.g.loaded_tau_nvim = 1

local tau = require("tau")

-- Define :Tau user command
vim.api.nvim_create_user_command("Tau", function(opts)
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
    tau.chat(range_opts)
  else
    tau.prompt(args, range_opts)
  end
end, {
  nargs = "*",
  range = true,
  desc = "τ Tau Autonomous Agent: Prompt or Chat",
})

-- Define :TauChat user command
vim.api.nvim_create_user_command("TauChat", function()
  tau.chat()
end, {
  desc = "τ Tau Autonomous Agent: Toggle Chat UI",
})

-- Define :TauExplain user command
vim.api.nvim_create_user_command("TauExplain", function(opts)
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

  tau.explain(range_opts)
end, {
  range = true,
  desc = "τ Tau: Explain Code or Diagnostic Context",
})

-- Define :TauFix user command
vim.api.nvim_create_user_command("TauFix", function(opts)
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

  tau.fix(range_opts)
end, {
  range = true,
  desc = "τ Tau: Fix Diagnostics or Selected Bugs",
})

-- Define :TauRefactor user command
vim.api.nvim_create_user_command("TauRefactor", function(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local range_opts = { bufnr = bufnr }
  local instruction = vim.trim(opts.args or "")

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

  range_opts.instruction = instruction
  tau.refactor(range_opts)
end, {
  nargs = "*",
  range = true,
  desc = "τ Tau: Refactor Code with Instructions",
})

-- Define :TauModels user command
vim.api.nvim_create_user_command("TauModels", function()
  tau.models()
end, {
  desc = "τ Tau: Interactive Model Switcher",
})

-- Define :TauStop user command
vim.api.nvim_create_user_command("TauStop", function()
  tau.stop()
end, {
  desc = "τ Tau: Cancel Active Generation / Turn",
})

-- Default <leader>t* keymaps
local map = vim.keymap.set
local map_opts = { silent = true, noremap = true }

map("n", "<leader>ta", function() tau.chat() end, vim.tbl_extend("force", map_opts, { desc = "Tau: Toggle Agent Chat" }))
map("n", "<leader>tc", function() tau.chat() end, vim.tbl_extend("force", map_opts, { desc = "Tau: Toggle Chat Window" }))
map("n", "<leader>te", function() tau.explain() end, vim.tbl_extend("force", map_opts, { desc = "Tau: Explain Buffer / Context" }))
map("n", "<leader>tf", function() tau.fix() end, vim.tbl_extend("force", map_opts, { desc = "Tau: Fix Diagnostics" }))
map("n", "<leader>tm", function() tau.models() end, vim.tbl_extend("force", map_opts, { desc = "Tau: Select Model" }))
map("n", "<leader>ts", function() tau.stop() end, vim.tbl_extend("force", map_opts, { desc = "Tau: Stop Generation" }))

map("v", "<leader>te", function() tau.explain({ use_selection = true }) end, vim.tbl_extend("force", map_opts, { desc = "Tau: Explain Selection" }))
map("v", "<leader>tf", function() tau.fix({ use_selection = true }) end, vim.tbl_extend("force", map_opts, { desc = "Tau: Fix Selection Diagnostics" }))
map("v", "<leader>tr", function() tau.refactor({ use_selection = true }) end, vim.tbl_extend("force", map_opts, { desc = "Tau: Refactor Selection" }))
