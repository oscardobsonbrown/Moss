#!/usr/bin/env python3
"""Clone the task-owned million-row storage fixture into a disposable app data folder."""
import argparse
import json
import pathlib
import sqlite3
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("--source", required=True, type=pathlib.Path)
parser.add_argument("--output", required=True, type=pathlib.Path)
args = parser.parse_args()
source = args.source.resolve(strict=True)
output = args.output.resolve()
if not str(output).startswith("/private/tmp/") and not str(output).startswith("/tmp/"):
    raise SystemExit("Output must be a new temporary folder")
if output.exists():
    raise SystemExit("Output folder already exists")
with sqlite3.connect(f"file:{source}?mode=ro&immutable=1", uri=True) as conn:
    counts = conn.execute("select (select count(*) from pages), (select count(*) from records), (select count(*) from blocks)").fetchone()
    if counts != (100000, 1000000, 299997):
        raise SystemExit(f"Unexpected fixture counts: {counts}")
output.mkdir(parents=True)
dest = output / "workspace.sqlite3"
subprocess.run(["/bin/cp", "-c", str(source), str(dest)], check=True)
with sqlite3.connect(dest) as conn:
    db = json.loads(conn.execute("select data from databases where id='bench-database'").fetchone()[0])
    for i in range(4):
        db["properties"].append({"id": f"extra-{i}", "name": f"Extra {i}", "kind": "text", "config": {}})
    db["views"] = [{"id": "bench-table", "name": "Table", "kind": "table", "query": {"filter": None, "sorts": [], "group_by": None, "limit": 100, "offset": 0}, "config": {"load_limit": 100, "show_page_icon": True}}]
    conn.execute("update databases set data=? where id=?", (json.dumps(db), db["id"]))
    rows = conn.execute("select id, data from records where database_id=? order by id limit 1200", (db["id"],)).fetchall()
    for row_id, raw in rows:
        record = json.loads(raw)
        record["title"] = f"Real row {row_id} — long title for wrapped table measurement " + "with useful text " * 8
        for i in range(4):
            record["values"][f"extra-{i}"] = f"Property {i}: sample data for {row_id} " + "wrapped content " * 4
        conn.execute("update records set title=?, values_json=?, data=? where id=?", (record["title"], json.dumps(record["values"]), json.dumps(record), row_id))
    conn.commit()
(output / "preferences.json").write_text(json.dumps({"workspace": "bench-workspace", "tabs": {"bench-workspace": ["page-000000000"]}, "active": {"bench-workspace": 0}}))
print(f"Scratch fixture: {output}; source counts: {counts}; prepared rows: {len(rows)}")
