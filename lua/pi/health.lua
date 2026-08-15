local M = {}

local rpc = require("pi.rpc")

local health = vim.health or {}
local report_start = health.start or health.report_start or function(name) vim.fn["health#report_start"](name) end
local report_ok = health.ok or health.report_ok or function(msg) vim.fn["health#report_ok"](msg) end
local report_warn = health.warn or health.report_warn or function(msg) vim.fn["health#report_warn"](msg) end
local report_error = health.error or health.report_error or function(msg) vim.fn["health#report_error"](msg) end
local report_info = health.info or health.report_info or function(msg) vim.fn["health#report_info"](msg) end

function M.check()
  report_start("pi.nvim Core System & Environment")

  -- 1. Neovim version check
  if vim.fn.has("nvim-0.8.0") == 1 then
    report_ok(string.format("Neovim version %s meets requirement (>= 0.8.0)", tostring(vim.version())))
  else
    report_error("Neovim version < 0.8.0 is not officially supported. Please upgrade Neovim.")
  end

  -- 2. Check pi-rs binary
  local config = require("pi").get_config()
  local bin_name = config.bin or "pi-rs"
  local bin_path = rpc.find_binary(bin_name)

  if bin_path then
    report_ok(string.format("pi-rs binary located at: %s", bin_path))

    -- Check binary execution
    local version_out = vim.fn.system({ bin_path, "--help" })
    if vim.v.shell_error == 0 then
      report_ok("pi-rs executable responds cleanly to CLI invocations.")
    else
      report_warn("pi-rs returned non-zero exit code on '--help': " .. tostring(version_out))
    end
  else
    report_error(
      string.format(
        "pi-rs binary '%s' not found in PATH or standard cargo target paths.\nBuild with `cargo build --release` and add target/release/ to PATH or configure `require('pi').setup({ bin = '/path/to/pi-rs' })`.",
        bin_name
      )
    )
  end

  -- 3. Check JSON-RPC 2.0 ping and communication
  report_start("pi-rs JSON-RPC 2.0 Daemon Health")
  if bin_path then
    local test_client = rpc.new({ bin = bin_path, model = config.model })
    local ok, err = test_client:start()
    if not ok then
      report_error("Failed to spawn pi-rs in --rpc mode: " .. tostring(err))
    else
      local ping_err, ping_res = test_client:request_sync("pi/ping", {}, 3000)
      if ping_err then
        report_warn("RPC 'pi/ping' request failed or timed out: " .. tostring(ping_err))
      else
        report_ok(
          string.format("RPC daemon active and responding to 'pi/ping' (version: %s)", tostring((ping_res and ping_res.version) or "ok"))
        )
      end

      -- Check model query
      local models_err, models_res = test_client:request_sync("pi/models", { force = false }, 4000)
      if models_err then
        report_warn("RPC 'pi/models' query timed out: " .. tostring(models_err))
      else
        local count = (models_res and models_res.models and #models_res.models) or 0
        report_ok(string.format("RPC daemon returned %d registered model configurations.", count))
      end

      test_client:stop()
    end
  else
    report_warn("Skipping RPC checks because pi-rs executable was not found.")
  end

  -- 4. Check configuration & active model
  report_start("pi.nvim Configuration & Providers")
  report_info(string.format("Default model configured: '%s'", config.model or "opencode/deepseek-v4-flash-free"))
  report_info(string.format("Keymaps enabled: %s", tostring(config.keymaps)))

  -- 5. Check Project Context & Skills
  report_start("pi.nvim Context & Environment")
  local cwd = vim.fn.getcwd()
  report_info(string.format("Current workspace: %s", cwd))

  local agents_md_path = vim.fn.filereadable(cwd .. "/AGENTS.md") == 1
  if agents_md_path then
    report_ok("Detected project-level AGENTS.md instructions in workspace root.")
  else
    report_info("No AGENTS.md found in workspace root (optional).")
  end

  local lsp_clients = vim.lsp.get_clients and vim.lsp.get_clients() or vim.lsp.get_active_clients()
  if #lsp_clients > 0 then
    report_ok(string.format("%d active LSP client(s) available for code intelligence & diagnostics.", #lsp_clients))
  else
    report_info("No LSP clients currently attached to active buffer (diagnostics will be fetched on demand).")
  end
end

return M
