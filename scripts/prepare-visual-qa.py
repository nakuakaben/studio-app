#!/usr/bin/env python3
"""Prepare the existing fictional catalogue for a five-card QA row (no OBS)."""
import json
from pathlib import Path
root = Path(__file__).resolve().parents[1]
out = root / "target" / "visual-qa"
out.mkdir(parents=True, exist_ok=True)
catalog = json.loads((root / "catalog/catalog.json").read_text())
(out / "catalog.json").write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
(out / "config.json").write_text(json.dumps({"source": {"kind": "local", "path": "catalog.json"}, "demo_scenarios": False}))
print(out / "config.json")
