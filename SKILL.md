# Blooket Claw Skill

## Skill metadata

```yaml
name: blooket-claw
version: 1.0.0
description: >
  Automated Blooket game assistant — joins games, answers questions via Claude AI,
  tracks scores and positions. Supports auto (fully autonomous), hint (shows answers
  to human), and speed (maximum score) modes.
entry: uv run python py/blooket.py
requires:
  - python >= 3.10
  - uv
  - rust (for building the engine)
env:
  - ANTHROPIC_API_KEY
```

---

## Commands

| Command | Description |
|---|---|
| `blooket play <pin>` | Join and auto-complete a game |
| `blooket play <pin> --mode hint` | Show answers without auto-clicking |
| `blooket play <pin> --mode speed` | Maximum speed auto-answer |
| `blooket check <pin>` | Validate a game PIN |
| `blooket ask "<question>" --choices "A,B,C,D"` | Ask Claude one question |
| `blooket interactive` | Live interactive assistant mode |
| `blooket history` | View session history |
| `blooket config` | Show current config |

---

## Prompt examples

```
Join Blooket game 123456 and play automatically
Check if Blooket PIN 654321 is active
What's the answer to: "What planet is closest to the sun?" with choices Mercury, Venus, Mars, Earth
Show me my Blooket history
Start Blooket interactive mode
Play Blooket PIN 999888 in hint mode
```

---

## Architecture

```
blooket-claw/
├── src/              ← Rust engine (WebSocket + HTTP + answer selection)
│   ├── main.rs       ← CLI entry point
│   ├── lib.rs        ← Shared types
│   ├── client.rs     ← Blooket API client
│   ├── game.rs       ← Game loop orchestrator
│   ├── answer.rs     ← Answer selection + LLM calls
│   ├── crypto.rs     ← Auth helpers
│   └── types.rs      ← Data structures
├── py/               ← Python layer (OpenClaw integration + Claude API)
│   ├── blooket.py    ← CLI dispatcher (Typer)
│   └── claude_helper.py ← Direct Claude AI integration
├── bin/              ← Compiled Rust binary (after build)
├── install.sh        ← macOS one-command installer
├── Cargo.toml        ← Rust dependencies
└── pyproject.toml    ← Python dependencies
```

---

## Modes

| Mode | Behavior | Best for |
|---|---|---|
| `auto` | Claude answers every question automatically with human-like delay | Unattended play |
| `hint` | Shows correct answer highlighted, you click it | Assisted play |
| `speed` | Auto-answer with minimal delay for maximum score | Tournaments |

---

## Signal tiers (answer confidence)

| Score | Meaning |
|---|---|
| ≥ 95% | Definitive — answer known from question set |
| 80–95% | High confidence LLM answer |
| 60–80% | Moderate — LLM uncertain |
| < 60% | Low — random fallback |
