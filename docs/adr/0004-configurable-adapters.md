# ADR 0004: Configurable harness adapters and bot defaults

Status: accepted (2026-11-17)

## Decision

SuperBot keeps native Rust adapters for Claude Code, Codex, and Pi, and adds a
persisted registry of custom CLI adapters. An administrator can create and edit
custom adapters through Settings & Adapters in the web app without rebuilding the
binary. The server stays responsible for spawning, cancelling, event streaming,
session ownership, memory, and bot-to-bot operations.

Custom definitions specify a program and argument arrays, prompt delivery,
optional model and effort arguments, model discovery, session strategy, output
format, and declarative JSONL event mappings. The server does not interpolate
commands into a shell. Only the authorized administrator can write settings and
adapter definitions; bot bearer tokens cannot use those management endpoints.

The primary bot, primary bot's harness, and default harness for new bots are
separate settings. Default harness changes persist in SQLite; the environment
variable is a first-run fallback. A harness change clears session, model, and
effort settings but preserves memory, chat history, and routines. Changing a
running bot's harness is rejected.

## Limitations

This supports agents with compatible non-interactive CLI interfaces. Agents
requiring a proprietary interactive protocol need a new generic transport
driver or a native adapter; arbitrary harnesses are not automatically compatible.

Plain-text CLIs have no structured tool or session events. Stateless adapters
receive bounded recent conversation history alongside the bot's system prompt
on each run. JSONL adapters can map supported event types and session IDs.
Model discovery requires a command that exits and emits a supported model
listing. No new shell or executable is downloaded by the app.

Native adapters remain code-based and can serve as stable fallbacks. Custom
definitions may emulate them when their CLI and event formats fit the generic
contract. A future protocol driver (e.g., ACP) can be added without changing
this user-facing registry.

## Security

Adapter administration is a privileged server-execution capability. Do not
enable it for untrusted users. A configured executable and model-discovery
command run with the privileges of botd. Use a dedicated non-root service user,
HTTPS, AUTH_PASSWORD, and host-level isolation. Argument substitution does not
invoke a shell, but an administrator can explicitly configure a shell program.
Existing bot harnesses also have broad local execution privileges; this ADR
does not add a sandbox.
