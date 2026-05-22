#!/usr/bin/env python3
"""Phase A monitor — a small curses TUI over the Backend protocol.

Two modes:
    LIST   — file roster with cursor. Default.
    DETAIL — full-screen scrollable event log for the selected file.

Usage:
    ./harness/monitor/monitor.py            # live
    ./harness/monitor/monitor.py --mock     # synthetic data for UI development

Keys (LIST mode):
    q          quit
    r          force refresh
    j / k      move selection (down / up)
    g / G      jump to first / last file
    Enter / e  open DETAIL mode for the selected file

Keys (DETAIL mode):
    q / Esc    back to LIST mode
    j / k      scroll one line
    d / u      scroll half a screen
    PgDn/PgUp  scroll a full screen
    g / G      jump to top / bottom
    r          force refresh
"""

from __future__ import annotations

import argparse
import curses
import textwrap
import time
from pathlib import Path

from backend import Backend, Event, FileEntry, LiveBackend, MockBackend, Summary


GLYPH = {
    "wait": "·",
    "work": "▶",
    "done": "✓",
    "fail": "✗",
    "skip": "−",
}

COLOR_PAIR = {
    "wait": 1,
    "work": 2,
    "done": 3,
    "fail": 4,
    "skip": 5,
}

EVENT_COLOR = {
    "init":   (7, curses.A_DIM),
    "text":   (2, 0),
    "think":  (6, 0),
    "tool":   (5, 0),
    "result": (7, curses.A_DIM),
    "done":   (3, curses.A_BOLD),
}


def init_colors() -> None:
    curses.start_color()
    curses.use_default_colors()
    curses.init_pair(1, curses.COLOR_WHITE,  -1)
    curses.init_pair(2, curses.COLOR_YELLOW, -1)
    curses.init_pair(3, curses.COLOR_GREEN,  -1)
    curses.init_pair(4, curses.COLOR_RED,    -1)
    curses.init_pair(5, curses.COLOR_CYAN,   -1)
    curses.init_pair(6, curses.COLOR_BLUE,   -1)
    curses.init_pair(7, curses.COLOR_WHITE,  -1)


def fmt_duration(s: int | None) -> str:
    if s is None or s < 0:
        return " — "
    m, sec = divmod(int(s), 60)
    return f"{m:>3}:{sec:02d}"


def fmt_cost(c: float | None) -> str:
    if c is None:
        return "    — "
    return f"${c:>5.2f}"


def truncate(s: str, n: int) -> str:
    if len(s) <= n:
        return s
    if n <= 1:
        return s[:n]
    return s[: n - 1] + "…"


def safe_addnstr(win, y: int, x: int, s: str, n: int, attr: int = 0) -> None:
    """addnstr that ignores the harmless ERR raised when writing to the
    bottom-right cell (curses advances the cursor past the window after the
    last char and reports ERR even though the glyph was drawn)."""
    try:
        win.addnstr(y, x, s, n, attr)
    except curses.error:
        pass


class UIState:
    def __init__(self) -> None:
        self.selected: int = 0
        self.row_offset: int = 0
        self.mode: str = "list"            # "list" or "detail"
        self.detail_line_offset: int = 0   # offset into the *wrapped* line list


def clamp_selection(ui: UIState, n_files: int, visible_rows: int) -> None:
    if n_files == 0:
        ui.selected = 0
        ui.row_offset = 0
        return
    ui.selected = max(0, min(ui.selected, n_files - 1))
    if ui.selected < ui.row_offset:
        ui.row_offset = ui.selected
    elif ui.selected >= ui.row_offset + visible_rows:
        ui.row_offset = ui.selected - visible_rows + 1
    ui.row_offset = max(0, min(ui.row_offset, max(0, n_files - visible_rows)))


def wrap_event_lines(events: list[Event], width: int) -> list[tuple[str, int, bool]]:
    """Flatten a list of Event objects into renderable lines.

    Returns a list of (text, color_pair_idx, is_first_line) where each Event
    becomes one tag line followed by zero or more wrapped continuation lines.
    Width is the available text width (caller subtracts indent already).
    """
    out: list[tuple[str, int, bool]] = []
    indent = "        "
    wrap_width = max(20, width - len(indent))
    for ev in events:
        cpair, attr = EVENT_COLOR.get(ev.type, (7, 0))
        tag = f"[{ev.type:<6}] "
        body = ev.summary or ""
        wrapped = textwrap.wrap(
            body, width=wrap_width,
            replace_whitespace=False, drop_whitespace=False,
            break_long_words=True, break_on_hyphens=False,
        ) or [""]
        first = tag + wrapped[0]
        out.append((first, cpair | (attr << 16), True))
        for cont in wrapped[1:]:
            out.append((indent + cont, cpair | (attr << 16), False))
    return out


def render_list(stdscr, files: list[FileEntry], summary: Summary,
                ui: UIState, refresh_pulse: bool) -> None:
    max_y, max_x = stdscr.getmaxyx()

    pulse = "↻ " if refresh_pulse else "  "
    clock = time.strftime("%H:%M:%S", time.localtime())
    title = f" {pulse if refresh_pulse else ' '}Phase A Monitor — lua-rs-port"
    safe_addnstr(stdscr, 0, 0, title.ljust(max_x - len(clock) - 1) + clock,
                 max_x, curses.color_pair(6) | curses.A_BOLD)

    elapsed = fmt_duration(summary.elapsed_s).strip()
    summary_line = (
        f" {summary.done_count} done · {summary.fail_count} fail · "
        f"{summary.work_count} work · {summary.wait_count} wait · "
        f"{summary.skip_count} skip   |   "
        f"elapsed {elapsed}   |   spent ${summary.total_cost:.2f}"
    )
    safe_addnstr(stdscr, 1, 0, summary_line[:max_x], max_x, curses.A_DIM)

    header = "    ST  FILE              TARGET                            COST    DUR    HK SX"
    safe_addnstr(stdscr, 3, 0, header[:max_x], max_x, curses.A_BOLD)
    safe_addnstr(stdscr, 4, 0, ("─" * max_x)[:max_x], max_x, curses.A_DIM)

    list_top = 5
    list_bottom = max_y - 2
    visible_rows = max(1, list_bottom - list_top + 1)
    clamp_selection(ui, len(files), visible_rows)

    for vis_i in range(visible_rows):
        i = ui.row_offset + vis_i
        if i >= len(files):
            break
        f = files[i]
        row = list_top + vis_i
        glyph = GLYPH.get(f.status, "?")
        cp = curses.color_pair(COLOR_PAIR.get(f.status, 1))
        target_short = f.target.replace("crates/", "").replace("/src/", "/")
        hk = "  " if f.hooks_pass is None else ("✓ " if f.hooks_pass else "✗ ")
        sx = "  " if f.syntax_ok is None else ("✓ " if f.syntax_ok else "✗ ")
        cursor = "▌" if i == ui.selected else " "
        line = (
            f"{cursor}  {glyph}  "
            f"{truncate(f.cfile, 17):<17} "
            f"{truncate(target_short, 33):<33} "
            f"{fmt_cost(f.cost_usd):>7} "
            f"{fmt_duration(f.duration_s):>6}  "
            f"{hk}{sx}"
        )
        attr = cp
        if i == ui.selected:
            attr |= curses.A_REVERSE
        safe_addnstr(stdscr, row, 0, line[:max_x], max_x, attr)

    footer = (
        " q quit · r refresh · j/k select · g/G top/bot · "
        "Enter open detail · auto 1s"
    )
    safe_addnstr(stdscr, max_y - 1, 0, footer[:max_x - 1], max_x - 1,
                 curses.A_REVERSE)


def render_detail(stdscr, sel: FileEntry, events: list[Event],
                  ui: UIState, refresh_pulse: bool) -> None:
    max_y, max_x = stdscr.getmaxyx()

    clock = time.strftime("%H:%M:%S", time.localtime())
    pulse = "↻ " if refresh_pulse else "  "
    glyph = GLYPH.get(sel.status, "?")
    status_color = curses.color_pair(COLOR_PAIR.get(sel.status, 1)) | curses.A_BOLD
    title = f" {pulse if refresh_pulse else ' '}{glyph} {sel.cfile} → {sel.target}"
    cost = fmt_cost(sel.cost_usd).strip()
    dur = fmt_duration(sel.duration_s).strip()
    right = f"{cost}   {dur}   {clock}"
    safe_addnstr(stdscr, 0, 0,
                 title.ljust(max(0, max_x - len(right) - 1)) + right,
                 max_x, status_color)

    safe_addnstr(stdscr, 1, 0, ("─" * max_x)[:max_x], max_x, curses.A_DIM)

    body_top = 2
    body_bottom = max_y - 2
    body_h = max(1, body_bottom - body_top + 1)

    lines = wrap_event_lines(events, max_x)
    total = len(lines)

    if total == 0:
        safe_addnstr(stdscr, body_top, 0,
                     " (no events yet — transcript not started)",
                     max_x, curses.A_DIM)
    else:
        max_off = max(0, total - body_h)
        ui.detail_line_offset = max(0, min(ui.detail_line_offset, max_off))
        window = lines[ui.detail_line_offset:ui.detail_line_offset + body_h]
        for i, (text, packed, _) in enumerate(window):
            cpair = packed & 0xFFFF
            attr_bits = (packed >> 16) & 0xFFFF
            safe_addnstr(stdscr, body_top + i, 0, text[:max_x], max_x,
                         curses.color_pair(cpair) | attr_bits)

    if total > body_h:
        first = ui.detail_line_offset + 1
        last = min(total, ui.detail_line_offset + body_h)
        pos = f" {first}–{last} / {total} "
        safe_addnstr(stdscr, max_y - 2, max(0, max_x - len(pos) - 1),
                     pos, len(pos), curses.A_REVERSE)

    footer = " q/Esc back · j/k line · d/u half · PgUp/PgDn page · g/G top/bot · r refresh"
    safe_addnstr(stdscr, max_y - 1, 0, footer[:max_x - 1], max_x - 1,
                 curses.A_REVERSE)


def render(stdscr, files: list[FileEntry], summary: Summary,
           ui: UIState, refresh_pulse: bool, backend: Backend) -> None:
    stdscr.erase()
    if ui.mode == "detail" and files:
        sel = files[min(ui.selected, len(files) - 1)]
        events = backend.events(sel.cfile, limit=1000)
        render_detail(stdscr, sel, events, ui, refresh_pulse)
    else:
        render_list(stdscr, files, summary, ui, refresh_pulse)
    stdscr.noutrefresh()
    curses.doupdate()


def loop(stdscr, backend: Backend, refresh_s: float) -> None:
    curses.curs_set(0)
    stdscr.nodelay(True)
    stdscr.timeout(int(refresh_s * 1000))
    init_colors()

    ui = UIState()
    refresh_pulse_until = 0.0
    while True:
        files = backend.files()
        summary = backend.summary()
        pulse = time.time() < refresh_pulse_until
        render(stdscr, files, summary, ui, pulse, backend)
        try:
            key = stdscr.getch()
        except KeyboardInterrupt:
            return

        max_y, _ = stdscr.getmaxyx()
        body_h = max(1, max_y - 4)

        if ui.mode == "list":
            if key in (ord("q"), 27):
                return
            elif key == ord("r"):
                refresh_pulse_until = time.time() + 0.8
            elif key in (ord("e"), 10, 13, curses.KEY_ENTER, curses.KEY_RIGHT):
                ui.mode = "detail"
                ui.detail_line_offset = 0
            elif key in (ord("j"), curses.KEY_DOWN):
                ui.selected = min(len(files) - 1, ui.selected + 1) if files else 0
            elif key in (ord("k"), curses.KEY_UP):
                ui.selected = max(0, ui.selected - 1)
            elif key == ord("g"):
                ui.selected = 0
            elif key == ord("G"):
                ui.selected = max(0, len(files) - 1)
        else:
            if key in (ord("q"), 27, curses.KEY_LEFT):
                ui.mode = "list"
                ui.detail_line_offset = 0
            elif key == ord("r"):
                refresh_pulse_until = time.time() + 0.8
            elif key in (ord("j"), curses.KEY_DOWN):
                ui.detail_line_offset += 1
            elif key in (ord("k"), curses.KEY_UP):
                ui.detail_line_offset = max(0, ui.detail_line_offset - 1)
            elif key == ord("d"):
                ui.detail_line_offset += body_h // 2
            elif key == ord("u"):
                ui.detail_line_offset = max(0, ui.detail_line_offset - body_h // 2)
            elif key == curses.KEY_NPAGE:
                ui.detail_line_offset += body_h
            elif key == curses.KEY_PPAGE:
                ui.detail_line_offset = max(0, ui.detail_line_offset - body_h)
            elif key == ord("g"):
                ui.detail_line_offset = 0
            elif key == ord("G"):
                ui.detail_line_offset = 10**9


def main() -> None:
    parser = argparse.ArgumentParser(description="Phase A monitor TUI.")
    parser.add_argument("--mock", action="store_true",
                        help="Use synthetic data; no harness/oracle/results/ required.")
    parser.add_argument("--root", type=Path,
                        default=Path(__file__).resolve().parents[2],
                        help="Project root (default: this script's grandparent).")
    parser.add_argument("--refresh", type=float, default=1.0,
                        help="Refresh interval in seconds (default 1.0).")
    args = parser.parse_args()

    backend: Backend = MockBackend() if args.mock else LiveBackend(args.root)
    curses.wrapper(loop, backend, args.refresh)


if __name__ == "__main__":
    main()
