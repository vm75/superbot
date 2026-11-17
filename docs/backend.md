# Backend

One Rust binary, `botd`. It serves the web app, stores everything in SQLite, and runs each Bot on a coding-agent CLI.

## How it works

- **Bots** live in SQLite. Each Bot has a harness (`claude`, `codex`, or `pi`), an optional model, and a harness session id.
- **A message** is queued. Each Bot runs one task at a time, in order. Different Bots run in parallel.
- **A run** spawns the harness CLI in the shared workspace, streams its JSON events, and saves each text block as a chat bubble.
- **Live updates** go to the browser over one Server-Sent Events stream: new messages, streaming text, and what the Bot is doing ("Running: ls").
- **Memory** is plain files, re-sent in the system prompt every run. The Bot edits them itself:
  - `DATA_DIR/bots/<id>/MEMORY.md`: one Bot's job, rules, and work log.
  - `DATA_DIR/USER.md`: facts about the user, shared by all Bots.
- **The computer** is `WORKSPACE_DIR`, shared by all Bots, like Grok Bot's shared VM.
- **Routines** are cron schedules. Bots create them with the `botctl` CLI, which works from any harness's shell.
- **Bot to Bot**: a Bot runs `botctl ask "<Bot>" "<message>"`. The other Bot answers in its own chat and the reply prints back. If it takes over 9 minutes, the reply arrives later as a new message.
- **Model and effort** are saved per Bot. The web app shows a faint picker above the input. Unset means the harness default.
- **Choice cards**: a reply ending in a ` ```question ` JSON block becomes a multiple-choice card.
- **Restarts**: queued messages run after a restart. A task that was running is reported as interrupted, not re-run, so it can't repeat an action like sending an email.

## Harness adapters

Each adapter is one file in `server/src/adapters/`. It builds the command line and maps the CLI's JSON lines to five events: session, text delta, text, tool, error.

- **Claude Code**: `claude -p --output-format stream-json --resume <id>`. System prompt via `--append-system-prompt`, fresh every run.
- **Codex**: `codex exec --json`, then `codex exec resume <id>`. Instructions via `-c developer_instructions=...`. Codex only reads them when a session starts, so later memory edits reach it through the memory file.
- **Pi**: `pi --mode json --session-dir <bot dir> --session-id <id>`. System prompt via `--append-system-prompt`.

All three run with permission prompts off (`--dangerously-skip-permissions` and equivalents). The server is the computer, so treat it that way. Approvals are not built yet.

### Configurable custom adapters

Open **Settings & Adapters** (the button at the bottom of the sidebar) to
add, edit, clone, check, disable or delete custom CLI adapters. The built-in
Claude Code, Codex and Pi adapters are retained unchanged.

**Starters:** Agy, Aider, and OpenCode buttons prefill editable definitions with
automatic model discovery. Agy uses `--print`; Aider uses `--message` with
plain output, noninteractive confirmations, separate history files, and automatic
commits disabled; OpenCode uses `run`, with model options after the subcommand.
All three include chat context on each run. Install and authenticate the CLI
as the server user before saving and selecting the adapter. Aider lists known
models, which can include providers without configured credentials.
See the [Aider options](https://aider.chat/docs/config/options.html) and
[OpenCode commands](https://opencode.ai/docs/cli/commands/) references.

To create an adapter, supply an ID, name, executable, arguments and output
format. The program must already be installed and usable by the botd service
user. This is not an installer or a code-plugin loader. Use noninteractive
CLIs: an interactive TTY agent may require a future transport driver.

The registry, primary-bot selection and default adapter for newly created
bots persist in SQLite. DEFAULT_HARNESS and PRIMARY_BOT_NAME are first-run
fallbacks. Changing the default never silently changes existing bots.
The primary bot's harness can be selected independently from the UI.

**Argument placeholders** are replaced within individual process arguments,
without a shell: `{{prompt}}`, `{{full_prompt}}`, `{{system}}`,
`{{model}}`, `{{effort}}`, `{{session_id}}`, `{{state_dir}}`,
and `{{cwd}}`. A multiline argument is still a single argument, not
multiple words. If stdin mode is enabled, the full prompt is also sent
to stdin. Model/effort arguments are only appended when selected, and are
placed before the base (positional) prompt arguments.

**Output** can be plain text or JSONL:
- Text: stdout lines become a live draft and one final chat reply.
  There is no tool or structured session telemetry.
- JSONL: define mappings such as the example below. Dot-separated
  value paths can address nested objects and array indexes. Supported
  event kinds: session, delta, text, tool, error.

```json
[
  {"kind":"session","when_path":"type","equals":"session","value_path":"sessionId"},
  {"kind":"delta","when_path":"type","equals":"delta","value_path":"text"},
  {"kind":"text","when_path":"type","equals":"message","value_path":"message.text"},
  {"kind":"tool","when_path":"type","equals":"tool","name_path":"name","detail_path":"command"},
  {"kind":"error","when_path":"type","equals":"error","value_path":"message"}
]
```

**Session strategies:** none, generated, or event. Stateless agents receive
the bot's rules, memory, and bounded recent conversation history each run.
For generated sessions, include the `{{session_id}}` placeholder in start
and resume arguments if the CLI supports an explicit ID. For event sessions,
provide a JSONL session event rule and resume arguments. Harness sessions
cannot be resumed across different agents.

**Models:** custom adapters automatically inspect CLI help for `--list-models`,
`models`, or `models list`. Discovery accepts JSON model arrays, one ID per
line, or tab-separated IDs and labels (including `agy models`). A documented
`--json` flag is used when available. Help inspection and listing share a
12-second timeout; results use the same ten-minute cache as built-in adapters.
Existing custom adapters gain this behavior without being recreated.

An explicit model-discovery command overrides automatic detection. Its output
may be JSON (array or object with a models array, IDs or model objects) or one
model ID per line, according to the selected format. Auto detect also accepts
tab-separated labels and bullet lists. Manual models, optionally
with labels/effort values, are the fallback when discovery yields none. CLIs
without a supported listing need an explicit command or manual models.
For example, an Agy starter profile can use program agy with --print and
`{{full_prompt}}` as two separate arguments. Verify that your installed
Agy version actually supports the chosen flags and authentication; the
starter is a template rather than a guaranteed compatibility adapter.

**Validation and safety:** the UI's Check button verifies executable
presence and permissions, not credentials or a successful agent run.
Actual agent execution happens when a bot sends a message. Administrator
settings/adapter mutation endpoints require browser Basic authentication
when AUTH_PASSWORD is configured; bot bearer tokens are rejected. When no
password is set, the server refuses to listen on a public address.
Custom CLI configuration is equivalent to granting server-side code
execution. Use only in a trusted single-user deployment with a non-root
service account and host isolation; arguments are not shell-evaluated.

Switching a bot's harness while it runs is blocked. When switched while
idle, its previous session ID, model and effort are cleared. Its memory,
chat history, and routines survive. On upgrade, known old Claude model
selections erroneously saved on Codex bots are reset automatically.

To add a *new protocol* rather than a compatible CLI, implement a shared
transport driver or native Rust adapter. The generic editor cannot decode
unknown interactive/binary protocols without code.


## Run it

```sh
make setup                    # creates .env without overwriting it
# Edit .env with your settings.
make run                      # builds both release binaries and loads .env
```

`make dev` runs a debug build. Use `make check`, `make test`, and `make lint`
for validation; `make help` lists all targets. `make run ENV_FILE=<path>` loads
a different environment file. Environment files use shell assignment syntax;
quote values containing spaces. Without a file, exported variables and server
defaults apply.

Settings are in [.env.example](../.env.example). Keep `botctl` next to `botd`: the server puts that folder on the Bots' PATH.

## Deploying on a Linux VPS

- Keep your server IP, domain, and secrets out of this repo. They go in `.env` on the server.

- Install the harness CLIs you want and log them in **as the user that runs `botd`**: `claude` (then `claude login` or `ANTHROPIC_API_KEY`), `codex` (`codex login` or `OPENAI_API_KEY`), `pi` (provider API key, e.g. `OPENROUTER_API_KEY`). Their env vars pass through to the Bots.
- Run `botd` as a normal user, not root. Claude Code refuses to skip permissions as root.
- Run it under systemd with `Restart=always`. It shuts down cleanly on SIGTERM.
- Put HTTPS in front (e.g. Caddy) and set `AUTH_PASSWORD` and `HOST=0.0.0.0`, or keep `HOST=127.0.0.1` behind the proxy. The browser logs in with any username and that password.
- Proxies must not buffer `/api/events` (it's an SSE stream).
- Set `TZ` to the user's time zone. Routines use it.
- Back up `DATA_DIR`. `WORKSPACE_DIR` is the Bots' scratch and files.
- `GET /healthz` returns `{"ok":true}` without auth.

## API

All JSON. Browser auth: HTTP Basic with `AUTH_PASSWORD`.

- `GET /api/state`: user name, installed harnesses, all Bots
- `GET /api/events`: SSE stream (`message`, `draft`, `status`, `bot`, `botDeleted`, `resync`)
- `POST /api/bots` `{name, harness?, model?, color?, shape?}`
- `PATCH /api/bots/:id` `{name?, hidden?, harness?, model?, effort?}`, `DELETE /api/bots/:id`
- `GET /api/harnesses/:name/models`: models and reasoning efforts the harness offers (Claude Code: aliases; Codex: `codex debug models`; Pi: `pi --list-models`)
- `GET|POST /api/bots/:id/messages` `{text, answer?: {messageId, option}}`
- `POST /api/bots/:id/read`, `POST /api/bots/:id/stop`
- `GET|POST /api/bots/:id/routines` `{name, cron, instruction}`
- `PATCH|DELETE /api/routines/:id` `{enabled?, name?, cron?, instruction?}`
- `GET /api/bots/:id/screen`: starts the Bot's screen and returns its live view URL
- `POST /api/handoff` `{from, text}`: `botctl handoff`; waits for the user's answer

## Not built yet

Approvals before risky actions, Vault for secrets, per-Bot resource limits, Slack.

Computer use (each Bot's screen, browser, and live view) is in [computer.md](computer.md).
