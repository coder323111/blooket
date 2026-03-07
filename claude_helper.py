"""
claude_helper.py — Direct Claude AI integration for Blooket Claw
Used when the Rust engine delegates LLM calls back to Python.
"""

import os
import json
import httpx
from typing import Optional


ANTHROPIC_API = "https://api.anthropic.com/v1/messages"

SYSTEM_PROMPT = """You are an expert quiz answering assistant with broad knowledge.
Given a multiple-choice question, identify the correct answer.
Respond ONLY with valid JSON (no markdown, no preamble):
{
  "answer_index": <int, 0-based index of correct answer>,
  "answer_text": "<exact text of correct answer>",
  "confidence": <float 0.0-1.0>,
  "reasoning": "<one brief sentence>"
}"""


def answer_question(
    question: str,
    choices: list[str],
    api_key: Optional[str] = None,
    model: str = "claude-sonnet-4-20250514",
) -> dict:
    """
    Ask Claude to answer a multiple-choice question.
    Returns dict with answer_index, answer_text, confidence, reasoning.
    """
    key = api_key or os.environ.get("ANTHROPIC_API_KEY", "")
    if not key:
        raise ValueError("ANTHROPIC_API_KEY not set")

    choices_text = "\n".join(f"{i}. {c}" for i, c in enumerate(choices))
    user_msg = f"Question: {question}\n\nChoices:\n{choices_text}\n\nWhat is the correct answer?"

    resp = httpx.post(
        ANTHROPIC_API,
        headers={
            "x-api-key": key,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json",
        },
        json={
            "model": model,
            "max_tokens": 256,
            "system": SYSTEM_PROMPT,
            "messages": [{"role": "user", "content": user_msg}],
        },
        timeout=30,
    )
    resp.raise_for_status()

    text = resp.json()["content"][0]["text"].strip()
    # Strip markdown fences
    clean = text.strip("`").removeprefix("json").strip()

    result = json.loads(clean)

    # Validate
    idx = int(result.get("answer_index", 0))
    if idx < 0 or idx >= len(choices):
        idx = 0

    return {
        "answer_index": idx,
        "answer_text": choices[idx],
        "confidence": float(result.get("confidence", 0.5)),
        "reasoning": result.get("reasoning", ""),
    }


def batch_answer(
    questions: list[dict],
    api_key: Optional[str] = None,
) -> list[dict]:
    """
    Answer a batch of questions. Each question dict has 'text' and 'choices'.
    Useful for pre-loading answers for an entire question set.
    """
    results = []
    for q in questions:
        try:
            result = answer_question(
                question=q["text"],
                choices=q["choices"],
                api_key=api_key,
            )
            results.append({**q, "result": result})
        except Exception as e:
            results.append({**q, "result": {"error": str(e)}})
    return results


if __name__ == "__main__":
    # Quick test
    import sys
    question = "What is the chemical symbol for gold?"
    choices = ["Ag", "Au", "Fe", "Cu"]
    result = answer_question(question, choices)
    print(f"Q: {question}")
    for i, c in enumerate(choices):
        mark = "✅" if i == result["answer_index"] else "  "
        print(f"  {mark} {i}. {c}")
    print(f"\nAnswer: {result['answer_text']} ({result['confidence']*100:.0f}% confident)")
    print(f"Reason: {result['reasoning']}")
