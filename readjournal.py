#!/usr/bin/env python3
"""Print the structured results recorded in a workflow journal."""
import json
import sys

PATH = ("/Users/istvan/.claude/projects/"
        "-Users-istvan-Developer-specforge--claude-worktrees-upgrade-deps/"
        "096c58a7-34f4-4c94-9167-267ea0733af0/subagents/workflows/"
        "wf_cc89439d-792/journal.jsonl")

want = sys.argv[1] if len(sys.argv) > 1 else None

with open(PATH) as fh:
    for line in fh:
        line = line.strip()
        if not line:
            continue
        rec = json.loads(line)
        if rec.get("type") != "result":
            continue
        val = rec.get("result")
        if isinstance(val, str):
            try:
                val = json.loads(val)
            except Exception:
                pass
        blob = json.dumps(val, indent=2)
        if want and want.lower() not in blob.lower():
            continue
        print("#" * 70)
        print(blob)
        print()
