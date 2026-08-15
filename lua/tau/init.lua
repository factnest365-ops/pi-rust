local pi = require("pi")

local M = {}

M.version = pi.version
M.setup = function(opts)
  opts = opts or {}
  opts.bin = opts.bin or "tau"
  return pi.setup(opts)
end

-- Re-export all functions from pi module
setmetatable(M, {
  __index = function(_, key)
    return pi[key]
  end,
})

return M
