"""np-drive: UI snapshot REPL, session runner, and driver for nix-pilot."""

from .snapshot import (
    Unit,
    Snapshot,
    parse_snapshot,
    load_snapshot,
    summarize,
    diff_snapshots,
    Summary,
    DiffResult,
    format_summary,
    format_diff,
)
from .session import Session, isolated_session, find_catalogue_index
from .driver import AtspiDriver, atspi_session, dump_atspi_state, click_unit, xdotool_click, xdotool_type, xdotool_key, screenshot
from .repl import Repl, create_repl, ReplState

__all__ = [
    # snapshot
    "Unit",
    "Snapshot",
    "parse_snapshot",
    "load_snapshot",
    "summarize",
    "diff_snapshots",
    "Summary",
    "DiffResult",
    "format_summary",
    "format_diff",
    # session
    "Session",
    "isolated_session",
    "find_catalogue_index",
    # driver
    "AtspiDriver",
    "atspi_session",
    "dump_atspi_state",
    "click_unit",
    "xdotool_click",
    "xdotool_type",
    "xdotool_key",
    "screenshot",
    # repl
    "Repl",
    "create_repl",
    "ReplState",
]

__version__ = "0.1.0"