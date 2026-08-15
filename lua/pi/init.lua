local actions = require("pi.actions")
local rpc = require("pi.rpc")

local M = {}

M.version = "0.1.0"

--- Default configuration table
M.defaults = {
  bin = "pi-rs",
  model = "opencode/deepseek-v4-flash-free",
  keymaps = true,
  floating_window = {
    width = 0.82,
    height = 0.82,
    border = "rounded",
  },
  auto_scroll = true,
  diff_preview = {
    auto_focus = true,
  },
}

--- Active runtime configuration
M.config = vim.deepcopy(M.defaults)

--- Setup pi.nvim with user configuration
---@param opts? table User options table
function M.setup(opts)
  opts = opts or {}
  M.config = vim.tbl_deep_extend("force", vim.deepcopy(M.defaults), opts)

  -- Initialize context with configured options
  actions.get_or_create_context(M.config)

  -- Register VimLeavePre autocommand for graceful daemon cleanup
  local augroup = vim.api.nvim_create_augroup("PiLifecycle", { clear = true })
  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = augroup,
    callback = function()
      local client = actions.get_client()
      if client then
        client:stop()
      end
    end,
  })

  -- Setup default keymaps if enabled
  if M.config.keymaps then
    M.setup_keymaps()
  end
end

--- Setup default keybindings
function M.setup_keymaps()
  local map = vim.keymap.set
  local opts = { silent = true, noremap = true }

  -- Normal mode keymaps
  map("n", "<leader>pa", function() M.chat() end, vim.tbl_extend("force", opts, { desc = "Pi: Toggle Agent Chat" }))
  map("n", "<leader>pc", function() M.chat() end, vim.tbl_extend("force", opts, { desc = "Pi: Toggle Chat Window" }))
  map("n", "<leader>pe", function() M.explain() end, vim.tbl_extend("force", opts, { desc = "Pi: Explain Buffer / Context" }))
  map("n", "<leader>pf", function() M.fix() end, vim.tbl_extend("force", opts, { desc = "Pi: Fix Diagnostics" }))
  map("n", "<leader>pm", function() M.models() end, vim.tbl_extend("force", opts, { desc = "Pi: Select Model" }))
  map("n", "<leader>ps", function() M.stop() end, vim.tbl_extend("force", opts, { desc = "Pi: Stop Generation" }))

  -- Visual mode keymaps
  map("v", "<leader>pe", function() M.explain({ use_selection = true }) end, vim.tbl_extend("force", opts, { desc = "Pi: Explain Selection" }))
  map("v", "<leader>pf", function() M.fix({ use_selection = true }) end, vim.tbl_extend("force", opts, { desc = "Pi: Fix Selection Diagnostics" }))
  map("v", "<leader>pr", function() M.refactor({ use_selection = true }) end, vim.tbl_extend("force", opts, { desc = "Pi: Refactor Selection" }))
end

--- Retrieve active configuration table
---@return table
function M.get_config()
  return M.config
end

--- Retrieve active RPC client instance
---@return table?
function M.get_client()
  return actions.get_client()
end

--- Check if RPC client is running
---@return boolean
function M.is_running()
  local client = actions.get_client()
  return client ~= nil and client:is_running()
end

-- Export core actions
M.chat = actions.chat
M.toggle = actions.chat
M.prompt = actions.prompt
M.explain = actions.explain
M.fix = actions.fix
M.refactor = actions.refactor
M.diff_and_apply = actions.diff_and_apply
M.models = actions.models
M.stop = actions.stop
M.replay = actions.replay

return M
