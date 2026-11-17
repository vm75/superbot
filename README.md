<p align="center">
  <img src="docs/images/superbot-hero.png" alt="SuperBot: open-source AI teammates with their own computer" width="100%">
</p>

<p align="center">
  <b>A team of AI Bots that work for you. Each one has a name, a job, memory, and a real computer.</b><br>
  Open source. Self-hosted on your VPS. Runs on the coding agent you already use.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-111?style=flat-square" alt="MIT license"></a>
  <img src="https://img.shields.io/badge/built%20in-Rust-e8702e?style=flat-square" alt="Built in Rust">
  <img src="https://img.shields.io/badge/runs%20on-Claude%20Code%20%7C%20Codex%20%7C%20Pi-3b82f6?style=flat-square" alt="Runs on Claude Code, Codex, Pi">
  <img src="https://img.shields.io/badge/status-early-4cc06a?style=flat-square" alt="Status: early">
</p>

> An open-source alternative to Grok Bot. Not affiliated with xAI or X. "Grok" is a trademark of xAI.

---

<p align="center">
  <img src="docs/images/superbot-app.png" alt="The SuperBot web app: a sidebar of robot Bots and a chat with the main Bot" width="100%">
</p>

You message Bots like teammates. They finish work end to end and only ask you when they need approval.

## What you get

<p align="center">
  <img src="docs/images/superbot-features.png" alt="Features: a team of Bots, memory, routines, Bot to Bot, any agent, approvals (soon)" width="100%">
</p>

- **A team of Bots**: named teammates, each with one job, its own chat, and its own memory.
- **Memory that grows**: Bots save your rules and preferences as plain files. Ask "What do you remember about me?"
- **Routines**: run a Bot on a schedule. Bots create their own cron jobs.
- **Bot to Bot**: Bots ask each other for help and pass work along.
- **Any CLI agent**: Claude Code, Codex and Pi built in; create custom CLI adapters in the web UI (including an Agy starter).
- **Approvals** *(soon)*: Bots pause and ask before risky actions.

Also planned: computer use (Bots drive the browser and GUI apps), skills, and Slack.

## How it works

<p align="center">
  <img src="docs/images/superbot-how-it-works.png" alt="You message SuperBot, it hands work to the team of Bots, and they all share one Linux computer" width="100%">
</p>

1. You talk to **SuperBot**, your main Bot.
2. It hands work to focused Bots, like Developer, Researcher, or Inbox.
3. Each Bot runs on the agent you picked for it.
4. They all share **one Linux computer** on your VPS. Files and sign-ins stick around between tasks.

## Meet the bots

<p align="center">
  <img src="docs/images/superbot-bots.png" alt="The 12 SuperBot robot avatars: waddle, drop, cloud, bean, onigiri, peanut, pebble, square, jelly, gem, gear, heart" width="100%">
</p>

Every Bot gets a robot. Pick one of 12 shapes and 11 colors when you create it.

## Quick start

```sh
cargo build --release          # makes target/release/botd and target/release/botctl
cp .env.example .env           # fill it in
set -a; . ./.env; set +a
./target/release/botd
```

Then open the web app and say hi to SuperBot. Install and log in to the agent CLIs you want (`claude`, `codex`, `pi`) as the same user that runs `botd`.

Deploying on a VPS: see [Backend](docs/backend.md).

## Harness agnostic

Bots run on the coding agent you choose:

- Claude Code CLI (first)
- Codex CLI
- Pi Agent
- Grok Build *(planned)*
- Hermes Agent *(planned)*

Each harness plugs in through one small adapter. Everything else stays the same.

## Built in Rust

As much as possible is Rust: the server, adapters, and CLI. That means speed, low memory use, and one small binary to self-host. Web code is used only where the browser needs it.

## The architecture we're replicating

Our rough guess at how Grok Bot works under the hood: one shared Linux VM per account, and every Bot gets its own little world on it. Based on what a Grok Bot told us about its own VM. Not official.

![How Grok Bot probably works under the hood](docs/images/grok-bot-architecture.png)

Details: [VM specs](docs/background/grok-bot-computer-specs.md) and [running many Bots on one VM](docs/background/shared-vm-architecture.md).

## Docs

- [Grok Bot reference](docs/grok-bot-reference.md): what we are rebuilding.
- [Architecture](docs/architecture.md): the plan and open decisions.
- [Backend](docs/backend.md): how the server works, configurable CLI adapters, and how to run and deploy it.

## Status

Early. Chat with Bots on Claude Code, Codex, or Pi works, with memory, routines, and Bot to Bot. Approvals and computer use are next.

## License

MIT
