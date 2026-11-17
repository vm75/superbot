# ADR 0005: Automatic model discovery for custom adapters

Status: accepted (2026-11-17)

## Decision

Custom adapters with no model-discovery override inspect their executable's
`--help` output. They use an advertised `--list-models`, `models`, or
`models list` command, preferring `--json` when that command advertises it.
Discovery accepts JSON model arrays, one ID per line, and tab-separated IDs
and display names. This includes the listing returned by Agy.

The implementation stays in the custom Rust adapter. No harness-specific
commands are added to the server or browser. Existing custom definitions
gain discovery without migration. Explicit discovery commands take priority;
manual models remain the fallback. The existing ten-minute model cache applies.

Starter definitions live in the Rust adapter layer and are served to the editor.
Aider's starter supplies a nonempty provider filter and disables Git and update
checks during discovery. Flags requiring unspecified arguments are skipped by
generic detection. Start/resume arguments precede model and effort options so
subcommands such as OpenCode's `run` receive their own flags.

Help inspection and discovery share a twelve-second timeout, use closed stdin,
and never invoke a shell. Commands are selected from advertised help rather
than guessed. An installed CLI must still support noninteractive model listing
and have credentials available to the server user.

## Limitations

There is no universal model-list protocol. Unsupported help layouts or output
formats still require an explicit command or manual models. Discovery does not
invent model IDs or reuse another harness's catalog.
