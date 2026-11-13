# Deprecations and breaking changes

[← Back to the README](../README.md)

What changed in a way that can need a step from you, with that step.

## Deprecations

A deprecated item keeps working next to its replacement. It warns once per run and names the replacement. Each one
gets a row here: its id, the old way, the replacement, your step and the aisbx version that deprecated it. None is
deprecated now.

## Breaking changes

A break comes from an sbx release that removed what aisbx needs, where no aisbx change can keep the old behavior.

| Break | sbx version | Your step |
| --- | --- | --- |
| aisbx 0.53.0.0 needs sbx 0.53.0 or later and refuses an older sbx. | 0.53.0 | Upgrade sbx to 0.53.0 or later, then check it with `sbx version`. |
| sbx refuses to create a sandbox whose name differs from an existing sandbox's name only in upper and lower case. `aisbx up --name NAME` then exits 2 and names the other sandbox. Without `--name`, `aisbx up` and the dashboard take the next free name, such as `claude-anthropic-Proj-2` next to `claude-anthropic-proj`. A rebuild of a sandbox whose name another sandbox has in other case is refused. | 0.53.0 | Pick another `--name`. To rebuild a sandbox with such a twin, delete the other one first ([Names](sandboxes.md#names)). |
