# Deprecations and breaking changes

[← Back to the README](../README.md)

What changed in a way that can need a step from you, with that step.

## Deprecations

A deprecated item keeps working next to its replacement. It warns once per run and names the replacement. Each one
gets a section here: its id, the old way, the replacement, your step and the aisbx version that deprecated it.

### `skill-repos`

| | |
| --- | --- |
| Old way | The skill repositories: `aisbx skill-repo`, `aisbx skill`, `aisbx up --skill`, and the dashboard's *Repo skills (deprecated)* rows on `K` and on `i`. |
| Replacement | The sbx skills store with a switch per skill: `K`, *Sandbox skills* (`i`), the create wizard's *shared skills* fold, [`aisbx skills`](headless.md#aisbx-skills) and `aisbx up --skill-on` ([Switches](skills.md#switches)). |
| Your step | Add each repository on `K` with `a` (every skill comes in, off, and the repository is kept in step), or with `sbx skills add REPO` and `aisbx skills update --track REPO`. Switch the skills on for all sandboxes on `K`, or for one sandbox on `i`. Then remove the repository with `aisbx skill-repo rm REPO`. |
| Deprecated in | 0.54.0.0 |

The commands keep working. `aisbx skill-repo` and `aisbx skill` print one warning on stderr per run; stdout does not
change. The pre-start sync of the picks warns nothing.

### `mcp-default`

| | |
| --- | --- |
| Old way | `aisbx up` created a sandbox in sbx's dynamic MCP mode: the agent could find and add the host's MCP servers. The create wizard opened with no MCP answer ticked, the same mode. |
| Replacement | MCP is off by default: `aisbx up` passes `--static-mcp none`, and the wizard opens with *No MCP servers* ticked. `aisbx up --mcp` and unticking *No MCP servers* give the old mode ([MCP servers](sandboxes.md#mcp-servers), [`up` options](headless.md)). |
| Your step | Add `--mcp` to each `aisbx up` that relies on the agent reaching the host's MCP servers. `--no-mcp` stays accepted and changes nothing; both flags together exit 2. In the wizard, untick *No MCP servers* once: the wizard remembers it. |
| Deprecated in | the first release after 0.54.0.0 |

Sandboxes that exist, the wizard's saved answers and rebuilds do not change. A saved empty MCP answer stays the
dynamic mode. A create with neither flag prints one note on stderr; stdout does not change.

## Breaking changes

A break comes from an sbx release that removed what aisbx needs, where no aisbx change can keep the old behavior, or
from a decision of aisbx, named as such.

| Break | sbx version | Your step |
| --- | --- | --- |
| aisbx 0.54.0.0 switches every skill of the sbx skills store off, also the skills already in it: a sandbox gets a store skill only when you switch it on. Before, every sharing sandbox got every store skill. This is a decision of aisbx, not of sbx. | none (aisbx) | Press `K` and switch on the skills every sandbox should get, or press `i` on a sandbox to switch skills on for it alone ([Switches](skills.md#switches)). `aisbx skills on --global SKILL` does the same in a script. |
| aisbx 0.54.0.0 needs sbx 0.54.0 or later and refuses an older sbx. | 0.54.0 | Upgrade sbx to 0.54.0 or later, then check it with `sbx version`. |
| aisbx 0.53.0.0 needs sbx 0.53.0 or later and refuses an older sbx. | 0.53.0 | Upgrade sbx to 0.53.0 or later, then check it with `sbx version`. |
| sbx refuses to create a sandbox whose name differs from an existing sandbox's name only in upper and lower case. `aisbx up --name NAME` then exits 2 and names the other sandbox. Without `--name`, `aisbx up` and the dashboard take the next free name, such as `claude-anthropic-Proj-2` next to `claude-anthropic-proj`. A rebuild of a sandbox whose name another sandbox has in other case is refused. | 0.53.0 | Pick another `--name`. To rebuild a sandbox with such a twin, delete the other one first ([Names](sandboxes.md#names)). |
