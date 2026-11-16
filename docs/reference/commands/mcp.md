# `collar mcp`

Global options apply to every command below; see the [command index](index.md).

```text
Serve Collar tools to MCP clients over stdio

Usage: collar mcp [OPTIONS]

Options:
      --allow-writes
          Expose tools that create or modify Forgejo resources

  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar mcp
  collar mcp --allow-writes
  collar -R terrier mcp

Speaks the Model Context Protocol on stdin and stdout for clients that cannot run shell commands. Tools run this binary with --json and share its credentials and validation. Read-only tools are exposed by default; --allow-writes adds PR authoring and reviews, issue authoring, and issue label updates. -R sets the default repository for repository tools.
```
