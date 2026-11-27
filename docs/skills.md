# Shared agent skills

[← Back to the README](../README.md)

Skills teach an agent how to do a task, for example how to fill in a PDF form or how your team reviews code. A
skill is a folder with a `SKILL.md` file. aisbx gives your skills to the agents in the sandboxes.

## Quick start

1. Press `K` (**Skills** in `aisbx-ui`).
2. Choose *Add from a Git repository* for a repository such as `anthropics/skills` (recommended: the skills then
   update daily). *Import this host's skills* copies your own skills (`~/.claude/skills`, `~/.agents/skills`, ...),
   but they do not update.
3. Switch on only the skills that every sandbox needs: `space` on a skill. Start with most of them off. A new skill
   always starts off.
4. Switch on more skills for one sandbox when you create it (the wizard's *skills*), or later with `i` on the
   sandbox.

The agent gets the change at its next start. On a running sandbox, *Apply now* on `i` links the skills at once. The
*Overview* tab of each sandbox names the skills its agent gets: press `i` there to change them.

aisbx still supports two older ways. Both are deprecated:

- the [`host-skills`](addons.md#host-skills-read-only) addon: the host's skills folders, read-only. Import those
  skills on `K` instead ([Deprecations](deprecations.md#host-skills)).
- [skill repositories](#skill-repositories-aisbx-skill-repo): Git repositories that aisbx mirrors. Add those skills
  to the store instead ([Deprecations](deprecations.md#skill-repos)).

## The sbx skills store

sbx keeps one skills store on the host and mounts it into each new sandbox. The store survives sandbox deletion.
aisbx decides which of its skills each agent gets.

### The skills screen (`K`)

Press `K` to open *Skills · all sandboxes*. Switch on here only the skills that every sandbox needs; a sandbox gets
more skills at its creation or on `i`. The skills are grouped by where they came from
([Where a skill came from](#where-a-skill-came-from)).
Each skill has a switch at the right: `● on` or `○ off`. Under the skills are *Add skills* and *Updates*.

| Key | Does |
| --- | --- |
| `space` or `↵` | switches the skill under the cursor on or off for all sandboxes |
| `a` | adds every skill of a Git repository, such as `anthropics/skills`, and keeps it in step ([Updates](#updates)). Recommended |
| `i` | imports your skill folders (`~/.agents/skills`, `~/.claude/skills`, `~/.config/opencode/skills`, ...). Not recommended: they do not update |
| `u` | updates the skills that `a` added now (*Update now*), as the daily update does |
| `d` | removes a skill |

The same screen has the *Update daily* switch ([Updates](#updates)). An import over a repository skill drops that
skill's link to its repository. After an import or an add, the status line names the skills that are new to the
store; they are off until you switch them on.

Without the dashboard, use `sbx skills import`, `sbx skills add <repo> --skill <name>`, `sbx skills update` and
`sbx skills ls`, and [`aisbx skills`](headless.md#aisbx-skills) for the switches.

### Switches

Every skill starts off: no sandbox gets it until you switch it on.

- **On:** every sandbox gets the skill at its next start, except a sandbox that switches it off.
- **Off:** only a sandbox that switches the skill on gets it.
- A row on `K` says how many sandboxes switch a skill the other way, for example *off in 1 sandbox*.

Each sandbox can switch single skills the other way:

- Press `i` on a sandbox to see the store's skills and the sandbox's switches. A switch that differs from the
  switch for all sandboxes is marked *set here*.
- Switch it back to follow the switch for all sandboxes again. *Reset to all sandboxes* drops every switch set here.
- On a running sandbox, *Apply now* links the switched-on skills at once. Otherwise the change arrives at the next
  start.
- `i` has *Add from a Git repository* and *Import this host's skills* too (keys `a` and `i`), as on `K`.
- In the create wizard, the *Skills* fold on the *Workspace and options* step sets the new sandbox's switches. It
  starts on the switches for all sandboxes. While the store holds no skill, the fold says so.

A rebuild keeps a sandbox's switches. Deleting the sandbox with `d` or `aisbx rm` drops them. The switches are in
`store-skills.json` in the aisbx state folder (`~/.local/state/aisbx`; on Windows `%LOCALAPPDATA%\aisbx\state`).

### When the switches apply

aisbx links the switched-on skills into the agent's skills folder before each agent start (`↵`, `aisbx run`), and
removes the links of the others. *Apply now* does the same for a running sandbox. The sandbox keeps that choice for
a later start with plain `sbx run`.

- A sandbox that an older aisbx created links every store skill at a plain `sbx run`. Rebuild it (`R`) to keep the
  choice there too. A start from the dashboard or with `aisbx run` uses the switches either way.
- A new sandbox gets its switches at its first start through aisbx: *Create & start*, `↵`, *Apply now* or
  `aisbx run`. After *Create only* or `aisbx up`, a plain `sbx run` before that links every store skill.
- A sandbox with the store `readwrite` gets every store skill: its skills folder is the store itself, so the switches
  do not apply.
- A sandbox with skills `off` gets no store skill.
- An agent that is running when a switch changes may find the change only at its next session.

### Where a skill came from

aisbx records where a skill came from when you import or add it on `K`:

- an import: the first of your skills folders that holds the skill, in the order `sbx skills import` reads them;
- an add: the repository, for each skill that the add brought into the store.

A skill without a record is listed under the host folder that holds a skill of its name. Otherwise it is under
*Other skills*: added with plain `sbx`, or imported from a folder that no longer holds it. sbx itself reports only
the skill names.

### Updates

aisbx keeps each repository you add on `K` (`a`) in step with the store. It mirrors the repository on the host,
compares its skills with the last update and changes the store:

- **New skills** of a repository you added whole come into the store. They start off: switch them on.
- **Changed skills** are replaced with the repository's version.
- **Deleted skills** leave the store, with their switches.

An add always takes every skill of the repository: they all start off, so you switch on only the ones you want, and
skills the repository gains later come in the same way. The add replaces skills of the same name in the store and
records them as the repository's. Later, a skill of the same name that you import or add from elsewhere is never
changed or deleted by the update. A skill you removed with `d` does not come
back. When you remove the last skill of a repository, aisbx stops updating it. A repository added with plain
`sbx skills add` is not kept in step: add it on `K` instead.

When it runs:

- **Update daily** (on by default): while the dashboard runs, once every 24 hours. Switch it off on `K`, or with
  `aisbx skills auto-update off`.
- *Update now* (`u`) runs it at once, then `sbx skills update` for skills added with plain `sbx`.
- `aisbx skills update` runs it from a script ([`aisbx skills`](headless.md#aisbx-skills)).

Each update reports what changed: on the status line, and under *Update daily* on `K` (*last update 2 hours ago:
1 changed*, and the lines on the row). An update that fails for a repository says why and changes nothing of it;
the next update tries again. A fetch never asks for credentials: a private repository needs your SSH key, the GitHub
CLI or `GITHUB_TOKEN`, as for [skill repositories](#sources-and-methods). An imported skill changes only when you
import it again.

**A skill's scripts are code: an update brings new code into every sandbox that has the skill on.**

### Share modes

Choose the mode on the *access* row of the create wizard's *Skills* fold. It starts on the host's
`skills.defaultMode`, which you change on the settings screen (`T`, *skills default*) or with
`sbx settings set skills.defaultMode <mode>`. The default also applies to `aisbx up` and a plain `sbx create` without
`--skills`. A rebuild keeps a sandbox's mode.

| Mode | The sandbox |
| --- | --- |
| `readonly` (default) | reads the skills that are switched on for it, but cannot change them |
| `readwrite` | reads every shared skill, and can change the skills that every other sharing sandbox loads |
| `off` | gets no store |

**`readwrite` puts every sharing sandbox behind one trust boundary.** A read-write agent can change the skills that
every other sandbox loads, and a skill holds scripts that agents run. So aisbx gives read-write only when you ask for
it on purpose:

- In the wizard and on `T`, `space`, `↵` and the arrows switch only between off and read-only. Read-write is `w` on
  the row (in `aisbx-ui`, a click on *Read-write*), and a red confirmation names the risk first.
- The wizard never starts on read-write: not from your last sandbox's answers, and not from a read-write host
  default.
- `aisbx up --skills readwrite`, or a read-write host default, prints a warning on stderr.
- A sandbox that is read-write says so in its details. *Make read-only* on its skills (`i`) rebuilds it read-only.

### Which backends get the store

Every backend gets the store:

| Backend | Skills linked into |
| --- | --- |
| `claude-litellm`, `claude-anthropic`, `claude-token`, `claude-local`, `claude-bare-local` | `/home/agent/.claude/skills` |
| `opencode-litellm`, `opencode-local` | `/home/agent/.config/opencode/skills` |
| `codex-openai`, `pi-litellm`, `pi-local` | `/home/agent/.agents/skills` |
| `agy` | `/home/agent/.gemini/antigravity/skills` and `/home/agent/.gemini/antigravity-cli/skills` |

An `agy` or Pi sandbox that an older aisbx created has no store. Rebuild it (`R`) to get one.

### How the store reaches the agent

The kits link each switched-on store skill into the agent's skills folder. An edit to a skill in the store reaches
the sandboxes that have it at once, through the link. A skill added to the store and switched on reaches a running
sandbox at its next start, or with *Apply now*. The agent's skills folder stays writable, and the agent's own skills
in it stay.

## Skill repositories (`aisbx skill-repo`)

**Deprecated:** add the repository on `K` instead (`a`): every skill comes in, off, and the repository is kept in
step. Then switch them on ([Switches](#switches)). The commands below keep working and warn on stderr. See
[Deprecations](deprecations.md#skill-repos).

aisbx keeps a list of skill repositories, public or private, and a local mirror of each. Sandboxes take skills from
the mirrors, never from the remote. Manage the list from the command line:

```console
$ aisbx skill-repo add acme/agent-skills                  # GitHub shorthand
$ aisbx skill-repo add git@github.com:acme/private-skills.git --ref v2
$ aisbx skill-repo fetch                                  # clone or update every repo
$ aisbx skill-repo ls                                     # id, source, ref, method, cached commit
$ aisbx skill-repo auto-update off                        # starts keep installed skills as they are
$ aisbx skill-repo rm acme/agent-skills                   # also deletes its mirror
```

Run `aisbx skill-repo fetch` to get a newer commit. Then pick the skills per sandbox
([Skills from skill repos](#skills-from-skill-repos-aisbx-skill)).

### Files

The list is `skill-repos.json` and the mirrors are `skill-cache/<id>/`, in the aisbx state folder.

### Sources and methods

A source is `owner/repo` (GitHub), `https://host/path`, `ssh://[user@]host/path` or `user@host:path`. `--ref` takes a
branch or tag. **Never paste a token into a source.**

| `--via` | How aisbx fetches |
| --- | --- |
| `ssh` | git over SSH with your keys |
| `gh` | the GitHub CLI; GitHub only |
| `token` | HTTPS to github.com with `GITHUB_TOKEN` |
| `git` | plain git with your own credential helper |
| `auto` (default) | tries `ssh`, `token`, `gh`, then `git` |

### The token rule

aisbx gives `GITHUB_TOKEN` to git only for the fetch. The token never goes into the list, the mirror or a message.

### Unattended by design

A fetch never prompts for credentials: without them it fails. ssh can still ask for a key passphrase when no agent
holds the key.

### `fetch`

A failed update keeps the previous commit. `fetch` exits 1 if a repo failed.

### The fetch report

`fetch` prints the skills that each repo added (`+`), removed (`-`) and changed (`~`). Then it tells, per sandbox,
what its next start does. `--json` prints the same report on stdout.

```text
acme-agent-skills-1a2b3c4d: 0123abcd4567 → 89abcdef0123: +pdf  -old (skipped: too-big)  ~docx
acme-agent-skills-1a2b3c4d:   probe: next start installs pdf; updates docx; removes old
```

### Auto-update

`auto-update on|off` is one switch for every sandbox. It is on by default. It does not fetch: only
`aisbx skill-repo fetch` does.

- **On:** the next start of each sandbox brings its installed skills to the fetched commit.
- **Off:** a start keeps each installed skill as it is. New picks still arrive and removed picks still go.
  `aisbx skill sync NAME` always updates.

To pin one repo instead, add it with `--ref <tag>`.

## Skills from skill repos (`aisbx skill`)

**Deprecated** with the skill repositories ([Deprecations](deprecations.md#skill-repos)).

Choose per sandbox which skills of the mirrored repos it gets. Global picks go to every sandbox. You need no kit and
no rebuild.

### In the dashboard

The dashboard offers these screens only while skill repositories are in use.

- Press `i` on a sandbox, then *Repo skills (deprecated)*, to pick its skills. Global picks show ticked and marked
  *global*; you change them only in the global picks.
- Press `K`, then *Repo skills (deprecated)*, to edit the global picks.
- Press `space` to tick a repo (*all skills*) or one skill. Then choose *Save*.
- Choose *Sync now* to copy the skills in at once. Otherwise they arrive at the next start.

### The selection

On the command line, a spec is `REPO` (every skill of the repo) or `REPO#SKILL` (one skill):

```console
$ aisbx up --backend claude-anthropic --skill acme/agent-skills#pdf --name probe ~/src/app
$ aisbx skill add probe acme/agent-skills                 # all of its skills, now and later
$ aisbx skill rm probe acme/agent-skills#pdf
$ aisbx skill add --global acme/agent-skills#review       # every sandbox
$ aisbx skill ls
$ aisbx skill sync probe                                  # copy them in now
```

aisbx refuses a skill name that two repos would give to one sandbox. The picks are in `skill-selection.json` in the
aisbx state folder.

### Global picks

Global picks reach every sandbox, also one made with plain `sbx`, at its next start through aisbx. A sandbox cannot
drop a global pick.

### What counts as a skill

A skill is a folder with a `SKILL.md`, at most four levels below the repo root. aisbx skips a folder with a bad name,
a symlink, a submodule or more than 20 MiB, and names each skip. **A skill's scripts are code: they run in the
sandbox when the agent uses them.**

### When it syncs

aisbx syncs the picks before each agent start (`↵`, `aisbx run`), on *Sync now* and on `aisbx skill sync`. A failed
sync does not stop the start. A rebuild keeps the picks; deleting the sandbox with `d` or `aisbx rm` drops them.

### In the sandbox

The skills are copied to `~/.local/share/aisbx-skills/` and linked into the agent's skills folders. The agent can
change the copies; the next sync overwrites them. No token or host path reaches the sandbox.
