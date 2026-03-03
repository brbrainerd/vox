# Decorators

Vox decorators provide metadata for the compiler and runtime.

| Decorator | Category | Description |
|-----------|----------|-------------|
| [@deprecated](decorators/deprecated.md) | function | Mark a function as deprecated. Emits a warning at every call site. |
| [@pure](decorators/pure.md) | function | Enforce function purity — no side effects allowed in the function body. |
| [@require](decorators/require.md) | function | Add a precondition assertion. Panics at runtime if the expression is false. |
| [@test](decorators/test.md) | function | Mark a function as a test case. Run with 'vox test'. |
| [@component](decorators/component.md) | ui | Define a React-like UI component that returns Element. |
| [@table](decorators/table.md) | data | Define a database table with typed fields. |
| [@index](decorators/index.md) | data | Define a database index on table fields. |
| [@query](decorators/query.md) | data | Read-only database function. Must have an explicit return type. |
| [@mutation](decorators/mutation.md) | data | Write database function with transaction semantics. |
| [@action](decorators/action.md) | data | Server-side logic that can call queries and mutations. |
| [@server](decorators/server.md) | infrastructure | Server-only function. Generates both a Rust handler and a TypeScript API client. |
| [@scheduled](decorators/scheduled.md) | infrastructure | Cron/interval scheduled function. |
| [@mcp.tool](decorators/mcp_tool.md) | infrastructure | Register a function as an MCP (Model Context Protocol) tool. |
| [@v0](decorators/v0.md) | ui | AI-generated component placeholder via v0.dev. |
