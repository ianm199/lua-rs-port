#!/usr/bin/env python3
"""Build a static lua-rs perf-history dashboard from the chassis evidence
ledger. Aesthetic and chart shape modeled after `redis-rs-port`'s
`harness/bench/history.py`.

Reads `harness/evidence/ledger.jsonl`, joins commit metadata via `git log`,
and writes a single self-contained HTML file with embedded JSON data + JS
charts. No external dependencies.

Output:
  harness/bench/history/index.html
  harness/bench/history/history.json

Usage:
  python3 harness/bench/history.py
  python3 harness/bench/history.py --open
"""

from __future__ import annotations

import argparse
import html
import json
import subprocess
import webbrowser
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
LEDGER = ROOT / "harness/evidence/ledger.jsonl"
OUT_DIR = ROOT / "harness/bench/history"
REMOTE_COMMIT_PREFIX = "https://github.com/ianm199/lua-rs-port/commit/"

WORKLOAD_COLORS = {
    "fibonacci":       "#2f6fed",
    "mandelbrot":      "#0f8f68",
    "binarytrees":     "#c16a1a",
    "closure_ops":     "#7a4cc2",
    "table_ops":       "#d33f49",
    "table_ops_long":  "#a32b35",
    "string_ops":      "#1a8caa",
    "string_ops_long": "#0d6478",
}

WORKLOAD_DESC = {
    "fibonacci":       "Recursive call dispatch + small-int math",
    "mandelbrot":      "Float math + nested loops",
    "binarytrees":     "GC pressure under steady allocation",
    "closure_ops":     "Closure allocation + upvalue access",
    "table_ops":       "Table insert/remove/iterate (short)",
    "table_ops_long":  "Table insert/remove/iterate (long, 50x scale)",
    "string_ops":      "String concat / find / gsub / byte (short)",
    "string_ops_long": "String concat / find / gsub / byte (long, 50x scale)",
}

PARITY_THRESHOLD = 1.5

INTERPRETER_FLOOR_NOTE = (
    "Safe-Rust interpreter floor sits around 1.5–2.0x for tight numeric "
    "loops; <=1.5x ratio counts as 'parity territory.'"
)


def commit_subject(sha: str) -> str:
    try:
        out = subprocess.check_output(
            ["git", "-C", str(ROOT), "log", "-1", "--format=%s", sha],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        return out
    except (subprocess.CalledProcessError, FileNotFoundError):
        return ""


def commit_ts_unix(sha: str) -> int:
    try:
        out = subprocess.check_output(
            ["git", "-C", str(ROOT), "log", "-1", "--format=%ct", sha],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        return int(out)
    except (subprocess.CalledProcessError, FileNotFoundError, ValueError):
        return 0


def load_bench_rows() -> list[dict[str, Any]]:
    if not LEDGER.exists():
        return []
    rows: list[dict[str, Any]] = []
    with LEDGER.open() as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                r = json.loads(line)
            except json.JSONDecodeError:
                continue
            if r.get("kind") == "bench" and r.get("target") == "rust-vs-reference":
                rows.append(r)
    rows.sort(key=lambda r: (r.get("ts", ""), r.get("commit", "")))
    return rows


def build_history(rows: list[dict[str, Any]]) -> dict[str, Any]:
    """Pivot ledger rows into the dashboard's expected shape.

    Output:
      {
        "commits": [{"sha", "subject", "ts_iso", "ts_unix"}, ...],
        "workloads": ["fibonacci", "mandelbrot", ...],
        "series": {
            "fibonacci": {
                "wall": [v0, v1, ...],
                "rss":  [v0, v1, ...],
            },
            ...
        },
        "summary": {
            "current_overall": float,
            "first_overall": float,
            "geomean_now": float,
            "geomean_first": float,
            "workloads_at_parity": int,
            "n_commits": int,
            ...
        },
      }
    """
    seen_commits: dict[str, dict[str, Any]] = {}
    for r in rows:
        sha = r["commit"]
        if sha not in seen_commits:
            ts_unix = commit_ts_unix(sha)
            seen_commits[sha] = {
                "sha": sha,
                "subject": commit_subject(sha),
                "ts_iso": r.get("ts", ""),
                "ts_unix": ts_unix,
            }
    commits = sorted(seen_commits.values(), key=lambda c: (c["ts_unix"], c["ts_iso"]))
    sha_order = [c["sha"] for c in commits]
    sha_idx = {sha: i for i, sha in enumerate(sha_order)}

    workloads = sorted({r["workload"] for r in rows})
    series: dict[str, dict[str, list]] = {
        w: {"wall": [None] * len(commits), "rss": [None] * len(commits)}
        for w in workloads
    }
    for r in rows:
        i = sha_idx[r["commit"]]
        w = r["workload"]
        m = r["metric"]
        if m == "wall_ratio":
            series[w]["wall"][i] = r["value"]
        elif m == "rss_ratio":
            series[w]["rss"][i] = r["value"]

    def first_last(values: list) -> tuple[float | None, float | None]:
        first = next((v for v in values if v is not None), None)
        last = next((v for v in reversed(values) if v is not None), None)
        return first, last

    workload_summary = []
    geo_first = 1.0
    geo_now = 1.0
    n_workloads = 0
    workloads_at_parity = 0
    best_workload = None
    worst_workload = None
    for w in workloads:
        first, last = first_last(series[w]["wall"])
        if first is None or last is None or first <= 0 or last <= 0:
            continue
        n_workloads += 1
        geo_first *= first
        geo_now *= last
        if last <= PARITY_THRESHOLD:
            workloads_at_parity += 1
        if best_workload is None or last < best_workload[1]:
            best_workload = (w, last)
        if worst_workload is None or last > worst_workload[1]:
            worst_workload = (w, last)
        workload_summary.append({
            "workload": w,
            "first": first,
            "last": last,
            "delta_pct": ((last / first) - 1.0) * 100.0 if first > 0 else None,
            "at_parity": last <= PARITY_THRESHOLD,
        })

    geomean_first = geo_first ** (1.0 / n_workloads) if n_workloads else 0
    geomean_now = geo_now ** (1.0 / n_workloads) if n_workloads else 0

    summary = {
        "n_commits": len(commits),
        "n_workloads": n_workloads,
        "workloads_at_parity": workloads_at_parity,
        "best_workload": best_workload,
        "worst_workload": worst_workload,
        "geomean_first": geomean_first,
        "geomean_now": geomean_now,
        "parity_threshold": PARITY_THRESHOLD,
    }

    return {
        "commits": commits,
        "workloads": workloads,
        "series": series,
        "summary": summary,
        "workload_summary": workload_summary,
    }


def render_metric_card(eyebrow: str, value: str, subtle: str = "", tone: str = "") -> str:
    tone_class = f" metric-{tone}" if tone else ""
    return (
        f'<article class="metric-card{tone_class}">'
        f'  <div class="eyebrow">{html.escape(eyebrow)}</div>'
        f'  <div class="metric">{html.escape(value)}</div>'
        f'  <div class="subtle">{subtle}</div>'
        f'</article>'
    )


def render_html(history: dict[str, Any]) -> str:
    summary = history["summary"]
    commits = history["commits"]
    workloads = history["workloads"]
    workload_summary = history["workload_summary"]

    if not commits:
        return (
            "<!doctype html><meta charset=utf-8>"
            "<title>lua-rs perf history</title>"
            "<body><p>No bench data yet. "
            "Run <code>bash harness/bench/compare.sh</code>.</p></body>"
        )

    # Hero metric cards
    geomean_now = summary["geomean_now"]
    geomean_first = summary["geomean_first"]
    geomean_pct = (geomean_now / geomean_first - 1.0) * 100.0 if geomean_first > 0 else 0
    overall_tone = "good" if geomean_now <= 2.0 else ""

    best_w, best_v = summary["best_workload"] or ("—", 0)
    worst_w, worst_v = summary["worst_workload"] or ("—", 0)
    cards = [
        render_metric_card(
            "Geomean ratio (latest)",
            f"{geomean_now:.2f}x",
            f"From {geomean_first:.2f}x at session start <span class='delta delta-good'>({geomean_pct:+.1f}%)</span>",
            tone=overall_tone,
        ),
        render_metric_card(
            "Workloads at parity",
            f"{summary['workloads_at_parity']} / {summary['n_workloads']}",
            f"&le; {PARITY_THRESHOLD}x of reference Lua 5.4.7",
            tone="good" if summary["workloads_at_parity"] > 0 else "",
        ),
        render_metric_card(
            "Best workload",
            f"{best_v:.2f}x",
            html.escape(best_w),
            tone="good" if best_v <= PARITY_THRESHOLD else "",
        ),
        render_metric_card(
            "Worst workload",
            f"{worst_v:.2f}x",
            html.escape(worst_w),
            tone="" if worst_v <= 3.0 else "warn",
        ),
        render_metric_card(
            "Tracked commits",
            f"{summary['n_commits']}",
            f"{len([w for w in workload_summary])} workloads &times; {summary['n_commits']} commits in ledger",
        ),
    ]

    # Workload-summary table rows
    ws_rows = []
    for ws in sorted(workload_summary, key=lambda x: x["last"]):
        w = ws["workload"]
        first = ws["first"]
        last = ws["last"]
        delta = ws["delta_pct"] or 0
        at_parity = ws["at_parity"]
        flag = "✓" if at_parity else ""
        flag_class = "parity-yes" if at_parity else ""
        delta_class = "delta-good" if delta <= 0 else "delta-bad"
        color = WORKLOAD_COLORS.get(w, "#888")
        desc = WORKLOAD_DESC.get(w, "")
        ws_rows.append(
            f'<tr>'
            f'<td><span class="swatch" style="background:{color}"></span> <strong>{html.escape(w)}</strong>'
            f' <span class="row-desc">{html.escape(desc)}</span></td>'
            f'<td>{first:.2f}x</td>'
            f'<td>{last:.2f}x</td>'
            f'<td class="{delta_class}">{delta:+.1f}%</td>'
            f'<td class="{flag_class}">{flag}</td>'
            f'</tr>'
        )

    # Recent commits table
    commit_rows = []
    for c in reversed(commits[-25:]):
        sha = c["sha"]
        sha_short = sha[:10]
        subject = html.escape(c.get("subject", "") or "")
        ts = c.get("ts_iso", "")[:19].replace("T", " ")
        url = f"{REMOTE_COMMIT_PREFIX}{sha}"
        commit_rows.append(
            f'<tr><td><a href="{url}" target="_blank"><code>{sha_short}</code></a></td>'
            f'<td>{ts}</td>'
            f'<td>{subject}</td></tr>'
        )

    # Embed full history as JSON for the JS to read
    history_json = json.dumps({
        "commits": commits,
        "workloads": workloads,
        "series": history["series"],
        "workload_colors": {w: WORKLOAD_COLORS.get(w, "#888") for w in workloads},
        "parity_threshold": PARITY_THRESHOLD,
    }, sort_keys=True)

    # Small-multiples grid: one mini-chart per workload
    mini_charts = []
    for w in workloads:
        color = WORKLOAD_COLORS.get(w, "#888")
        desc = WORKLOAD_DESC.get(w, "")
        latest = next((s for ws in workload_summary if ws["workload"] == w for s in [ws["last"]]), 0)
        delta = next((ws["delta_pct"] for ws in workload_summary if ws["workload"] == w), 0) or 0
        delta_class = "delta-good" if delta <= 0 else "delta-bad"
        at_parity = latest <= PARITY_THRESHOLD
        parity_badge = '<span class="parity-badge">parity</span>' if at_parity else ""
        mini_charts.append(f"""
        <article class="mini-card">
          <div class="mini-head">
            <div>
              <div class="mini-title"><span class="swatch" style="background:{color}"></span> {html.escape(w)} {parity_badge}</div>
              <div class="mini-desc">{html.escape(desc)}</div>
            </div>
            <div class="mini-stats">
              <div class="mini-now">{latest:.2f}x</div>
              <div class="mini-delta {delta_class}">{delta:+.1f}%</div>
            </div>
          </div>
          <svg class="mini-chart" data-workload="{html.escape(w)}" role="img"></svg>
        </article>
        """)

    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>lua-rs Performance History</title>
  <style>
    :root {{
      --bg: #f7f8fb;
      --panel: #ffffff;
      --text: #18202f;
      --muted: #5e6878;
      --line: #d8deea;
      --accent: #2f6fed;
      --good: #0f8f68;
      --warn: #c16a1a;
      --bad: #d33f49;
      --hover-bg: #f0f4ff;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; background: var(--bg); color: var(--text); -webkit-font-smoothing: antialiased; }}
    main {{ max-width: 1440px; margin: 0 auto; padding: 28px; }}
    header.page-header {{ margin-bottom: 28px; }}
    h1 {{ margin: 0 0 8px; font-size: 28px; letter-spacing: -0.01em; }}
    h2 {{ margin: 0 0 16px; font-size: 18px; letter-spacing: 0; }}
    p {{ margin: 0 0 8px; color: var(--muted); line-height: 1.5; }}
    a {{ color: var(--accent); text-decoration: none; }}
    a:hover {{ text-decoration: underline; }}
    code {{ font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 12px; }}

    .hero-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; margin-bottom: 24px; }}
    .metric-card {{ background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 16px; }}
    .metric-card.metric-good {{ border-color: #b9e6d3; background: linear-gradient(180deg, #f1faf6 0%, #ffffff 100%); }}
    .metric-card.metric-warn {{ border-color: #f0d8b0; background: linear-gradient(180deg, #fdf6ec 0%, #ffffff 100%); }}
    .eyebrow {{ color: var(--muted); font-size: 11px; text-transform: uppercase; letter-spacing: .08em; font-weight: 600; }}
    .metric {{ margin-top: 6px; font-size: 32px; font-weight: 700; line-height: 1.1; }}
    .subtle {{ color: var(--muted); font-size: 12px; margin-top: 6px; }}
    .delta {{ font-weight: 600; }}
    .delta-good {{ color: var(--good); }}
    .delta-bad {{ color: var(--bad); }}

    .panel {{ background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 20px; margin-bottom: 20px; }}
    .panel-head {{ display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 12px; }}
    .panel-head h2 {{ margin: 0; }}
    .panel-head .panel-meta {{ color: var(--muted); font-size: 12px; }}

    .chart-wrap {{ width: 100%; overflow-x: auto; }}
    svg.main-chart {{ display: block; width: 100%; min-width: 760px; height: 420px; }}
    .axis {{ stroke: #9aa6bb; stroke-width: 1; }}
    .gridline {{ stroke: #e9edf5; stroke-width: 1; }}
    .parity-line {{ stroke: var(--good); stroke-width: 1.5; stroke-dasharray: 4 4; }}
    .parity-line-label {{ fill: var(--good); font-size: 11px; font-weight: 600; }}
    .series-line {{ fill: none; stroke-width: 2.5; }}
    .series-line.dim {{ opacity: 0.15; }}
    .point {{ stroke: #fff; stroke-width: 1.5; }}
    .point:hover {{ stroke-width: 2.5; }}
    .axis-label {{ fill: var(--muted); font-size: 11px; }}

    .legend {{ display: flex; flex-wrap: wrap; gap: 8px 16px; margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--line); }}
    .legend-item {{ display: inline-flex; align-items: center; gap: 6px; color: var(--text); font-size: 13px; cursor: pointer; padding: 4px 8px; border-radius: 6px; transition: background 0.15s; }}
    .legend-item:hover {{ background: var(--hover-bg); }}
    .legend-item.dim {{ opacity: 0.4; }}
    .swatch {{ width: 11px; height: 11px; border-radius: 999px; display: inline-block; flex-shrink: 0; }}

    .mini-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(340px, 1fr)); gap: 12px; }}
    .mini-card {{ background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 14px; }}
    .mini-head {{ display: flex; justify-content: space-between; align-items: flex-start; gap: 12px; margin-bottom: 8px; }}
    .mini-title {{ font-weight: 600; font-size: 14px; display: flex; align-items: center; gap: 6px; }}
    .mini-desc {{ color: var(--muted); font-size: 11px; margin-top: 3px; }}
    .mini-stats {{ text-align: right; flex-shrink: 0; }}
    .mini-now {{ font-size: 22px; font-weight: 700; line-height: 1; }}
    .mini-delta {{ font-size: 11px; font-weight: 600; margin-top: 3px; }}
    .mini-chart {{ width: 100%; height: 90px; display: block; }}
    .parity-badge {{ display: inline-block; background: #d8f5e8; color: var(--good); border-radius: 6px; padding: 1px 6px; font-size: 10px; font-weight: 700; letter-spacing: 0.04em; text-transform: uppercase; }}

    table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
    th, td {{ text-align: left; border-bottom: 1px solid var(--line); padding: 9px 10px; vertical-align: top; }}
    th {{ color: var(--muted); font-weight: 600; font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; }}
    tr:last-child td {{ border-bottom: none; }}
    .row-desc {{ color: var(--muted); font-size: 11px; margin-left: 6px; }}
    .parity-yes {{ color: var(--good); font-weight: 700; }}

    .tooltip {{
      position: fixed;
      background: #18202f;
      color: #ffffff;
      padding: 8px 11px;
      border-radius: 6px;
      font-size: 12px;
      pointer-events: none;
      opacity: 0;
      transition: opacity 0.1s;
      z-index: 100;
      box-shadow: 0 4px 16px rgba(0,0,0,0.18);
      max-width: 320px;
    }}
    .tooltip.show {{ opacity: 1; }}
    .tooltip .tip-sha {{ color: #98aff1; font-family: ui-monospace, monospace; font-size: 11px; }}
    .tooltip .tip-subject {{ color: #c9d3e4; font-size: 11px; margin-top: 3px; line-height: 1.3; }}
    .tooltip .tip-value {{ font-weight: 700; font-size: 14px; margin-top: 4px; }}

    footer.page-footer {{ text-align: center; color: var(--muted); font-size: 12px; margin-top: 32px; padding-top: 18px; border-top: 1px solid var(--line); }}
  </style>
</head>
<body>
<main>
  <header class="page-header">
    <h1>lua-rs Performance History</h1>
    <p>
      Wall-clock ratio of the safe-Rust port (<code>target/release/lua-rs</code>) against pinned upstream Lua 5.4.7
      across {summary['n_commits']} commits and {summary['n_workloads']} workloads.
      Lower is better. The dashed line at <strong>1.0×</strong> is parity.
    </p>
    <p style="margin-top:8px;">{INTERPRETER_FLOOR_NOTE}</p>
  </header>

  <section class="hero-grid">
    {''.join(cards)}
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2>All workloads over time</h2>
      <div class="panel-meta">Click a legend entry to dim/show. Hover a point for commit details.</div>
    </div>
    <div class="chart-wrap"><svg class="main-chart" id="main-chart" role="img" aria-label="All workloads wall-clock ratio over commits"></svg></div>
    <div class="legend" id="main-legend"></div>
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2>Per-workload trajectory</h2>
      <div class="panel-meta">Small multiples; each panel is one workload over the same commit timeline.</div>
    </div>
    <div class="mini-grid">
      {''.join(mini_charts)}
    </div>
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2>Workload summary</h2>
      <div class="panel-meta">Session-start vs. latest. {summary['workloads_at_parity']} of {summary['n_workloads']} below the {PARITY_THRESHOLD}× parity threshold.</div>
    </div>
    <table>
      <thead><tr><th>Workload</th><th>First</th><th>Latest</th><th>Δ</th><th>Parity</th></tr></thead>
      <tbody>{''.join(ws_rows)}</tbody>
    </table>
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2>Recent commits</h2>
      <div class="panel-meta">Most recent first; click SHA to view on GitHub.</div>
    </div>
    <table>
      <thead><tr><th>SHA</th><th>UTC</th><th>Subject</th></tr></thead>
      <tbody>{''.join(commit_rows)}</tbody>
    </table>
  </section>

  <footer class="page-footer">
    Generated from <code>harness/evidence/ledger.jsonl</code> by <code>harness/bench/history.py</code>.
    Methodology in <code>docs/PERFORMANCE_PRINCIPLES.md</code> &amp; <code>docs/MATCHING_C_PERFORMANCE.md</code>.
  </footer>
</main>

<div class="tooltip" id="tip"></div>

<script>
const HISTORY = {history_json};

const tip = document.getElementById("tip");
function showTip(evt, workload, sha, subject, value, metric) {{
  tip.innerHTML = `
    <div><strong>${{workload}}</strong></div>
    <div class="tip-value">${{value.toFixed(2)}}x ${{metric}}</div>
    <div class="tip-sha">${{sha}}</div>
    <div class="tip-subject">${{subject}}</div>`;
  tip.style.left = (evt.clientX + 12) + "px";
  tip.style.top = (evt.clientY + 12) + "px";
  tip.classList.add("show");
}}
function hideTip() {{ tip.classList.remove("show"); }}

const SVG_NS = "http://www.w3.org/2000/svg";
function el(tag, attrs = {{}}, kids = []) {{
  const e = document.createElementNS(SVG_NS, tag);
  for (const k in attrs) e.setAttribute(k, attrs[k]);
  for (const c of kids) e.appendChild(c);
  return e;
}}

function drawMainChart() {{
  const svg = document.getElementById("main-chart");
  const W = svg.clientWidth || 1100;
  const H = 420;
  const pad = {{ l: 56, r: 18, t: 16, b: 36 }};
  const plotW = W - pad.l - pad.r;
  const plotH = H - pad.t - pad.b;

  const commits = HISTORY.commits;
  const series = HISTORY.series;
  const workloads = HISTORY.workloads;
  const n = Math.max(commits.length, 1);

  // Compute y range (log scale to show 1x to ~10x cleanly)
  let yMin = Math.log10(0.9);
  let yMax = Math.log10(2.0);
  for (const w of workloads) {{
    for (const v of series[w].wall) {{
      if (v == null || v <= 0) continue;
      const lv = Math.log10(v);
      if (lv < yMin) yMin = lv;
      if (lv > yMax) yMax = lv;
    }}
  }}
  yMin -= 0.04;
  yMax += 0.04;

  function xAt(i) {{ return pad.l + (n === 1 ? plotW/2 : (i / (n-1)) * plotW); }}
  function yAt(logv) {{ return pad.t + plotH - ((logv - yMin) / (yMax - yMin)) * plotH; }}

  svg.innerHTML = "";
  svg.setAttribute("viewBox", `0 0 ${{W}} ${{H}}`);

  // Y gridlines (at each integer log level: 1x, 2x, 5x, 10x, ...)
  const gridVals = [];
  for (let log = Math.floor(yMin); log <= Math.ceil(yMax); log++) {{
    for (const mant of [1, 2, 5]) {{
      const v = mant * Math.pow(10, log);
      const lv = Math.log10(v);
      if (lv < yMin || lv > yMax) continue;
      gridVals.push(v);
    }}
  }}
  for (const v of gridVals) {{
    const y = yAt(Math.log10(v));
    svg.appendChild(el("line", {{ class: "gridline", x1: pad.l, x2: pad.l + plotW, y1: y, y2: y }}));
    svg.appendChild(el("text", {{ class: "axis-label", x: pad.l - 8, y: y + 4, "text-anchor": "end" }}, [
      document.createTextNode(v + "x")
    ]));
  }}

  // Parity line at 1.0x
  const yOne = yAt(0);
  if (yOne >= pad.t && yOne <= pad.t + plotH) {{
    svg.appendChild(el("line", {{ class: "parity-line", x1: pad.l, x2: pad.l + plotW, y1: yOne, y2: yOne }}));
    svg.appendChild(el("text", {{ class: "parity-line-label", x: pad.l + plotW - 6, y: yOne - 4, "text-anchor": "end" }}, [
      document.createTextNode("parity (1.0x)")
    ]));
  }}

  // X labels at first, middle, last commit
  const xLabelIdxs = n <= 1 ? [0] : (n <= 4 ? [...Array(n).keys()] : [0, Math.floor(n/2), n-1]);
  for (const i of xLabelIdxs) {{
    const c = commits[i];
    const x = xAt(i);
    svg.appendChild(el("text", {{ class: "axis-label", x, y: H - pad.b + 16, "text-anchor": "middle" }}, [
      document.createTextNode(c.sha.slice(0, 7))
    ]));
  }}

  // Series
  for (const w of workloads) {{
    const color = HISTORY.workload_colors[w] || "#888";
    const values = series[w].wall;
    let path = "";
    let cmd = "M";
    for (let i = 0; i < values.length; i++) {{
      const v = values[i];
      if (v == null || v <= 0) {{ cmd = "M"; continue; }}
      const x = xAt(i);
      const y = yAt(Math.log10(v));
      path += `${{cmd}}${{x.toFixed(1)}},${{y.toFixed(1)}} `;
      cmd = "L";
    }}
    if (path) {{
      svg.appendChild(el("path", {{
        class: "series-line", d: path, stroke: color, "data-workload": w
      }}));
    }}
    // Points
    for (let i = 0; i < values.length; i++) {{
      const v = values[i];
      if (v == null || v <= 0) continue;
      const x = xAt(i);
      const y = yAt(Math.log10(v));
      const pt = el("circle", {{
        class: "point", cx: x.toFixed(1), cy: y.toFixed(1), r: 3.5, fill: color,
        "data-workload": w, "data-i": i
      }});
      const c = commits[i];
      pt.addEventListener("mouseenter", evt => showTip(evt, w, c.sha.slice(0,10), c.subject || "", v, "wall ratio"));
      pt.addEventListener("mousemove", evt => {{
        tip.style.left = (evt.clientX + 12) + "px";
        tip.style.top = (evt.clientY + 12) + "px";
      }});
      pt.addEventListener("mouseleave", hideTip);
      svg.appendChild(pt);
    }}
  }}

  // Legend
  const legend = document.getElementById("main-legend");
  legend.innerHTML = "";
  for (const w of workloads) {{
    const color = HISTORY.workload_colors[w] || "#888";
    const item = document.createElement("div");
    item.className = "legend-item";
    item.dataset.workload = w;
    item.innerHTML = `<span class="swatch" style="background:${{color}}"></span> ${{w}}`;
    item.addEventListener("click", () => {{
      item.classList.toggle("dim");
      const dimmed = item.classList.contains("dim");
      svg.querySelectorAll(`[data-workload="${{w}}"]`).forEach(el => {{
        if (el.tagName === "path") {{
          el.classList.toggle("dim", dimmed);
        }} else {{
          el.style.opacity = dimmed ? "0.15" : "1";
        }}
      }});
    }});
    legend.appendChild(item);
  }}
}}

function drawMiniCharts() {{
  document.querySelectorAll(".mini-chart").forEach(svg => {{
    const w = svg.dataset.workload;
    const W = svg.clientWidth || 300;
    const H = 90;
    const pad = {{ l: 4, r: 4, t: 6, b: 6 }};
    const plotW = W - pad.l - pad.r;
    const plotH = H - pad.t - pad.b;

    const values = HISTORY.series[w].wall;
    const n = Math.max(values.length, 1);
    const valid = values.filter(v => v != null && v > 0);
    if (valid.length === 0) return;
    let vMin = Math.log10(Math.min(...valid, 0.9));
    let vMax = Math.log10(Math.max(...valid, 2.0));
    vMin -= 0.05; vMax += 0.05;

    function xAt(i) {{ return pad.l + (n === 1 ? plotW/2 : (i / (n-1)) * plotW); }}
    function yAt(logv) {{ return pad.t + plotH - ((logv - vMin) / (vMax - vMin)) * plotH; }}

    svg.innerHTML = "";
    svg.setAttribute("viewBox", `0 0 ${{W}} ${{H}}`);

    // Parity line
    const yOne = yAt(0);
    if (yOne >= pad.t && yOne <= pad.t + plotH) {{
      svg.appendChild(el("line", {{
        x1: pad.l, x2: pad.l + plotW, y1: yOne, y2: yOne,
        stroke: "#0f8f68", "stroke-width": 1, "stroke-dasharray": "3 3", opacity: 0.5
      }}));
    }}

    const color = HISTORY.workload_colors[w] || "#888";
    let path = "";
    let cmd = "M";
    for (let i = 0; i < values.length; i++) {{
      const v = values[i];
      if (v == null || v <= 0) {{ cmd = "M"; continue; }}
      const x = xAt(i);
      const y = yAt(Math.log10(v));
      path += `${{cmd}}${{x.toFixed(1)}},${{y.toFixed(1)}} `;
      cmd = "L";
    }}
    svg.appendChild(el("path", {{
      d: path, fill: "none", stroke: color, "stroke-width": 1.8
    }}));

    // Last point highlight
    for (let i = values.length - 1; i >= 0; i--) {{
      const v = values[i];
      if (v == null || v <= 0) continue;
      const x = xAt(i);
      const y = yAt(Math.log10(v));
      svg.appendChild(el("circle", {{
        cx: x.toFixed(1), cy: y.toFixed(1), r: 3, fill: color, stroke: "#fff", "stroke-width": 1.5
      }}));
      break;
    }}
  }});
}}

window.addEventListener("DOMContentLoaded", () => {{
  drawMainChart();
  drawMiniCharts();
}});
window.addEventListener("resize", () => {{
  drawMainChart();
  drawMiniCharts();
}});
</script>
</body>
</html>
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--open", action="store_true")
    args = ap.parse_args()

    rows = load_bench_rows()
    history = build_history(rows)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    (OUT_DIR / "history.json").write_text(json.dumps(history, indent=2, sort_keys=True, default=str))
    html_out = render_html(history)
    out_path = OUT_DIR / "index.html"
    out_path.write_text(html_out)
    print(
        f"wrote {out_path} "
        f"({len(rows)} bench rows, {history['summary']['n_commits']} commits, "
        f"{history['summary']['n_workloads']} workloads, "
        f"geomean {history['summary']['geomean_now']:.2f}x)"
    )
    if args.open:
        webbrowser.open(out_path.as_uri())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
