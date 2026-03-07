#!/usr/bin/env python3
"""
blooket.py — Blooket Claw Python CLI
Orchestrates the Rust engine + Claude AI for automated Blooket sessions.
"""

import os
import sys
import json
import subprocess
import shutil
from pathlib import Path
from typing import Optional

import typer
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.prompt import Prompt, Confirm
from rich import box
from dotenv import load_dotenv

load_dotenv()

app = typer.Typer(
    help="🎮 Blooket Claw — automated Blooket assistant powered by Claude AI",
    rich_markup_mode="rich",
)
console = Console()

ENGINE_PATH = Path(__file__).parent / "bin" / "blooket-engine"
HISTORY_FILE = Path.home() / ".blooket-claw" / "history.json"


def find_engine() -> Optional[Path]:
    """Locate the compiled Rust engine binary."""
    candidates = [
        ENGINE_PATH,
        Path(__file__).parent.parent / "target" / "release" / "blooket-engine",
        Path(shutil.which("blooket-engine") or ""),
    ]
    for c in candidates:
        if c.exists():
            return c
    return None


def run_engine(args: list[str], api_key: str = "") -> dict:
    """Run the Rust engine with given args and return parsed result."""
    engine = find_engine()
    if not engine:
        console.print("[red]❌ blooket-engine binary not found.[/red]")
        console.print("[dim]Run: cargo build --release[/dim]")
        raise typer.Exit(1)

    env = os.environ.copy()
    if api_key:
        env["ANTHROPIC_API_KEY"] = api_key

    result = subprocess.run(
        [str(engine)] + args,
        env=env,
        capture_output=False,
        text=True,
    )

    return {"exit_code": result.returncode}


def save_history(result: dict):
    """Save session result to local history file."""
    HISTORY_FILE.parent.mkdir(parents=True, exist_ok=True)
    history = []
    if HISTORY_FILE.exists():
        try:
            history = json.loads(HISTORY_FILE.read_text())
        except Exception:
            pass
    history.append(result)
    HISTORY_FILE.write_text(json.dumps(history, indent=2))


# ─── Commands ─────────────────────────────────────────────────────────────────

@app.command("play")
def play(
    pin: str = typer.Argument(..., help="Blooket game PIN"),
    name: str = typer.Option("BlooketClaw", "--name", "-n", help="Player display name"),
    mode: str = typer.Option("auto", "--mode", "-m", help="Mode: auto | hint | speed"),
    delay: int = typer.Option(1500, "--delay", "-d", help="Base answer delay (ms)"),
    api_key: Optional[str] = typer.Option(None, "--api-key", envvar="ANTHROPIC_API_KEY"),
):
    """
    🎮 [bold green]Join and auto-complete a Blooket game[/bold green]

    Modes:
      [cyan]auto[/cyan]   — Fully automated: Claude answers every question
      [cyan]hint[/cyan]   — Shows you the correct answer, you click it
      [cyan]speed[/cyan]  — Auto-answer with minimal delay (max score)
    """
    key = api_key or os.environ.get("ANTHROPIC_API_KEY", "")

    console.print(Panel.fit(
        f"[bold cyan]🎮 Blooket Claw[/bold cyan]\n"
        f"PIN: [yellow]{pin}[/yellow]  |  Name: [green]{name}[/green]  |  Mode: [magenta]{mode}[/magenta]",
        border_style="cyan"
    ))

    if not key and mode != "hint":
        console.print("[yellow]⚠️  No ANTHROPIC_API_KEY set — LLM answers unavailable.[/yellow]")
        console.print("[dim]Set it with: export ANTHROPIC_API_KEY=sk-ant-...[/dim]\n")

    run_engine(
        ["play", "--pin", pin, "--name", name, "--mode", mode, "--delay", str(delay)],
        api_key=key,
    )


@app.command("check")
def check(pin: str = typer.Argument(..., help="Game PIN to validate")):
    """🔍 Check if a game PIN is valid and active."""
    run_engine(["check", pin])


@app.command("ask")
def ask(
    question: str = typer.Argument(..., help="Question text"),
    choices: str = typer.Option(..., "--choices", "-c", help="Comma-separated answer choices"),
    api_key: Optional[str] = typer.Option(None, "--api-key", envvar="ANTHROPIC_API_KEY"),
):
    """
    🤖 [bold]Ask Claude a single multiple-choice question[/bold]

    Example:
      blooket ask "What is the capital of France?" --choices "London,Paris,Berlin,Rome"
    """
    key = api_key or os.environ.get("ANTHROPIC_API_KEY", "")
    if not key:
        console.print("[red]❌ ANTHROPIC_API_KEY is required for this command.[/red]")
        raise typer.Exit(1)

    run_engine(["ask", question, "--choices", choices, "--api-key", key])


@app.command("history")
def history(last: int = typer.Option(10, "--last", "-n", help="Show last N sessions")):
    """📊 View your past Blooket Claw sessions."""
    if not HISTORY_FILE.exists():
        console.print("[dim]No session history yet.[/dim]")
        return

    try:
        sessions = json.loads(HISTORY_FILE.read_text())
    except Exception:
        console.print("[red]Could not read history file.[/red]")
        return

    sessions = sessions[-last:]

    table = Table(title="📊 Session History", box=box.ROUNDED)
    table.add_column("PIN", style="yellow")
    table.add_column("Name", style="cyan")
    table.add_column("Score", justify="right", style="bold green")
    table.add_column("Accuracy", justify="right")
    table.add_column("Mode", style="magenta")
    table.add_column("Duration")

    for s in reversed(sessions):
        table.add_row(
            s.get("pin", "?"),
            s.get("name", "?"),
            str(s.get("final_score", 0)),
            f"{s.get('accuracy', 0):.0f}%",
            s.get("mode", "?"),
            f"{s.get('duration_secs', 0)}s",
        )

    console.print(table)


@app.command("interactive")
def interactive():
    """
    🎯 [bold]Interactive assistant mode[/bold]

    Answer Blooket questions live — paste the question and choices,
    Claude tells you the correct answer instantly.
    """
    key = os.environ.get("ANTHROPIC_API_KEY", "")
    if not key:
        key = Prompt.ask("Enter your [cyan]ANTHROPIC_API_KEY[/cyan]", password=True)
        if not key:
            console.print("[red]API key required.[/red]")
            raise typer.Exit(1)

    console.print(Panel.fit(
        "[bold cyan]🎯 Blooket Claw — Interactive Mode[/bold cyan]\n"
        "[dim]Paste questions and get instant Claude answers.[/dim]\n"
        "[dim]Type 'quit' to exit.[/dim]",
        border_style="cyan"
    ))

    while True:
        console.print()
        question = Prompt.ask("[bold]❓ Question[/bold] (or 'quit')")
        if question.lower() in ("quit", "exit", "q"):
            console.print("[dim]Goodbye! 👋[/dim]")
            break

        choices_raw = Prompt.ask("[bold]📝 Choices[/bold] (comma-separated)")
        choices = [c.strip() for c in choices_raw.split(",") if c.strip()]

        if not choices:
            console.print("[yellow]Need at least one choice.[/yellow]")
            continue

        choices_str = ",".join(choices)
        run_engine(["ask", question, "--choices", choices_str, "--api-key", key])


@app.command("config")
def config():
    """⚙️  Show current configuration and environment."""
    engine = find_engine()
    api_key = os.environ.get("ANTHROPIC_API_KEY", "")

    table = Table(title="⚙️  Blooket Claw Config", box=box.ROUNDED)
    table.add_column("Setting")
    table.add_column("Value")

    table.add_row(
        "Engine",
        f"[green]{engine}[/green]" if engine else "[red]NOT FOUND[/red]",
    )
    table.add_row(
        "ANTHROPIC_API_KEY",
        f"[green]{api_key[:12]}...[/green]" if api_key else "[red]NOT SET[/red]",
    )
    table.add_row("History", str(HISTORY_FILE))
    table.add_row("Python", sys.version.split()[0])

    console.print(table)


if __name__ == "__main__":
    app()
