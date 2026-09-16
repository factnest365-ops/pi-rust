// hello.js - hand-stripped fallback of hello.ts (no TS types, no imports)
// Exposes a factory function registerHello(pi) mimicking `export default (pi) => ...`
function registerHello(pi) {
  pi.registerTool({
    name: "hello",
    label: "Hello",
    description: "A simple greeting tool",
    parameters: {
      type: "object",
      properties: {
        name: { type: "string", description: "Name to greet" }
      },
      required: ["name"]
    },
    execute: function (params) {
      return "Hello, " + params.name + "!";
    }
  });
}
