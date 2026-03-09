# Apps tool discovery

Searches over apps tool metadata with BM25 and exposes matching tools for the next model call.

MCP tools of the apps ({{app_names}}) are hidden until you search for them with this tool (`tool_search`).

Follow this workflow:

1. Call `tool_search` with:
   - `query` (required): focused terms that describe the capability you need.
   - `limit` (optional): maximum number of tools to return (default `8`).
2. Read the returned `tool_search_output.tools` namespaces to see the matching Apps tools grouped by app.
3. Choose the relevant child tool from the matching namespace and use that tool directly in the rest of the response flow.

Notes:
- Core tools remain available without searching.
- If you are unsure, start with `limit` between 5 and 10 to see a broader set of tools.
- `query` is matched against Apps tool metadata fields:
  - `name`
  - `tool_name`
  - `server_name`
  - `title`
  - `description`
  - `connector_name`
  - input schema property keys (`input_keys`)
- If the needed app is already explicit in the prompt (for example `[$app-name](app://{connector_id})`) or already present in the current `tools` list, you can call that tool directly.
- Do not use `tool_search` for non-apps/local tasks (filesystem, repo search, or shell-only workflows) or anything not related to {{app_names}}.
