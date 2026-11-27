#!/usr/bin/env python3
"""
Snapshot parsing and analysis for np-drive.

Parses the app's JSON UI snapshot (emitted via NIX_PILOT_UI_DUMP) and
provides querying, summarization, and diff utilities.
"""

import json
from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any
from pathlib import Path


@dataclass
class Unit:
    """A single UI unit from the snapshot."""
    id: int
    kind: str
    role: str
    name: str
    label: str
    x: int
    y: int
    w: int
    h: int
    visible: bool
    sensitive: bool
    focused: bool

    @property
    def center(self) -> tuple[int, int]:
        """Center point of the unit's bounds (for clicking)."""
        return (self.x + self.w // 2, self.y + self.h // 2)

    @property
    def rect(self) -> tuple[int, int, int, int]:
        """Bounds as (x, y, w, h)."""
        return (self.x, self.y, self.w, self.h)

    @property
    def is_interactive(self) -> bool:
        """Whether this unit is an interactive widget type."""
        return self.kind in ("button", "switch", "entry", "row")

    @property
    def is_unnamed(self) -> bool:
        """Whether this interactive unit has no accessible name."""
        return self.is_interactive and (not self.name or self.name.strip() == "")

    def contains_point(self, x: int, y: int) -> bool:
        """Check if a point is within this unit's bounds."""
        return self.x <= x < self.x + self.w and self.y <= y < self.y + self.h

    def is_offscreen(self, window_w: int, window_h: int) -> bool:
        """Check if the unit is entirely outside the window rect."""
        return (self.x + self.w <= 0 or self.x >= window_w or
                self.y + self.h <= 0 or self.y >= window_h)

    def is_effectively_visible(self, window_w: int, window_h: int) -> bool:
        """Whether the unit is really on screen.

        The app's raw `visible` flag is unreliable for hidden stack pages
        (unrealized units report a tiny default allocation); require a
        plausible size and on-window bounds on top of the flag.
        """
        return (self.visible and self.w >= 8 and self.h >= 8
                and not self.is_offscreen(window_w, window_h))

    def is_window_chrome(self, window_w: int, window_h: int) -> bool:
        """Whether this is a GTK titlebar window control.

        The headerbar's minimize/maximize/close buttons have no text label;
        their names come from the window manager, which our text-based
        emitter cannot see. Exclude them from the unnamed-unit audit.
        """
        return (self.kind == "button" and not self.name.strip()
                and self.w >= 16 and self.h >= 16
                and self.y <= 60
                and self.x + self.w >= window_w - 160)

    def is_unnamed_addressable(self, window_w: int, window_h: int) -> bool:
        """Whether this is an actionable control that lacks a name.

        Combines an interactive, unnamed unit with effective visibility,
        sensitivity, a known role and non-window-chrome. Hidden stack-page
        units, disabled controls and unassigned-role widgets therefore never
        count as naming gaps. Shared by the summary and the none-unnamed
        assert so the definition lives in one place.
        """
        return (self.is_unnamed
                and self.is_effectively_visible(window_w, window_h)
                and self.sensitive
                and not self.role.startswith("__Unknown")
                and not self.is_window_chrome(window_w, window_h))


@dataclass
class Snapshot:
    """Full UI snapshot with window geometry, page, and units."""
    window_x: int
    window_y: int
    window_w: int
    window_h: int
    page: Optional[str]
    units: List[Unit] = field(default_factory=list)

    @property
    def window_rect(self) -> tuple[int, int, int, int]:
        return (self.window_x, self.window_y, self.window_w, self.window_h)

    @property
    def interactive_units(self) -> List[Unit]:
        return [u for u in self.units if u.is_interactive]

    @property
    def visible_units(self) -> List[Unit]:
        return [u for u in self.units if u.visible]

    @property
    def hidden_units(self) -> List[Unit]:
        return [u for u in self.units if not u.visible]

    @property
    def effectively_visible_units(self) -> List[Unit]:
        return [u for u in self.units if u.is_effectively_visible(self.window_w, self.window_h)]

    @property
    def offscreen_units(self) -> List[Unit]:
        return [u for u in self.units if u.is_offscreen(self.window_w, self.window_h)]

    @property
    def focused_unit(self) -> Optional[Unit]:
        for u in self.units:
            if u.focused:
                return u
        return None

    def find_by_id(self, unit_id: int) -> Optional[Unit]:
        """Find a unit by its stable index id."""
        for u in self.units:
            if u.id == unit_id:
                return u
        return None

    def find_by_name(self, name: str, exact: bool = True) -> List[Unit]:
        """Find units by accessible name (exact or substring)."""
        if exact:
            return [u for u in self.units if u.name == name]
        else:
            return [u for u in self.units if name.lower() in u.name.lower()]

    def find_by_label_substring(self, substr: str) -> List[Unit]:
        """Find units whose label contains the substring (case-insensitive)."""
        return [u for u in self.units if substr.lower() in u.label.lower()]

    def find_by_kind(self, kind: str) -> List[Unit]:
        """Find units by widget kind."""
        return [u for u in self.units if u.kind == kind]

    def find_by_role(self, role: str) -> List[Unit]:
        """Find units by AT-SPI role."""
        return [u for u in self.units if u.role == role]


def parse_snapshot(json_text: str) -> Snapshot:
    """Parse a JSON snapshot string into a Snapshot object."""
    data = json.loads(json_text)
    window = data.get("window", {})
    page = data.get("page")
    units_data = data.get("units", [])

    units = []
    for u in units_data:
        units.append(Unit(
            id=u["id"],
            kind=u["kind"],
            role=u["role"],
            name=u["name"],
            label=u["label"],
            x=u["x"],
            y=u["y"],
            w=u["w"],
            h=u["h"],
            visible=u["visible"],
            sensitive=u["sensitive"],
            focused=u["focused"],
        ))

    return Snapshot(
        window_x=window.get("x", 0),
        window_y=window.get("y", 0),
        window_w=window.get("w", 0),
        window_h=window.get("h", 0),
        page=page,
        units=units,
    )


def load_snapshot(path: str) -> Snapshot:
    """Load and parse a snapshot from a file."""
    with open(path, "r") as f:
        return parse_snapshot(f.read())


@dataclass
class Summary:
    """Summary metrics for a snapshot."""
    counts_by_kind: Dict[str, int]
    unnamed_interactive: List[Unit]
    duplicate_names: Dict[str, List[Unit]]
    hidden_units: List[Unit]
    offscreen_units: List[Unit]
    focused_unit_id: Optional[int]
    visible_page: Optional[str]
    total_visible_text_length: int


def summarize(snapshot: Snapshot) -> Summary:
    """Compute summary metrics for a snapshot."""
    # Counts by kind
    counts_by_kind: Dict[str, int] = {}
    for u in snapshot.units:
        counts_by_kind[u.kind] = counts_by_kind.get(u.kind, 0) + 1

    # Actionable units that lack a name (hidden, disabled and unknown-role
    # widgets never count as gaps).
    unnamed_interactive = [
        u for u in snapshot.units
        if u.is_unnamed_addressable(snapshot.window_w, snapshot.window_h)
    ]

    # Duplicate names among interactive units
    name_to_units: Dict[str, List[Unit]] = {}
    for u in snapshot.interactive_units:
        if u.name and u.name.strip():
            name_to_units.setdefault(u.name, []).append(u)
    duplicate_names = {name: units for name, units in name_to_units.items() if len(units) > 1}

    hidden_units = snapshot.hidden_units

    offscreen_units = snapshot.offscreen_units

    focused = snapshot.focused_unit
    focused_unit_id = focused.id if focused else None

    visible_page = snapshot.page

    # Total visible text length (sum of label lengths of on-screen units)
    total_visible_text_length = sum(len(u.label) for u in snapshot.effectively_visible_units)

    return Summary(
        counts_by_kind=counts_by_kind,
        unnamed_interactive=unnamed_interactive,
        duplicate_names=duplicate_names,
        hidden_units=hidden_units,
        offscreen_units=offscreen_units,
        focused_unit_id=focused_unit_id,
        visible_page=visible_page,
        total_visible_text_length=total_visible_text_length,
    )


@dataclass
class DiffResult:
    """Result of diffing two snapshots."""
    appeared: List[Unit]           # Units in new but not in old (by id)
    disappeared: List[Unit]        # Units in old but not in new (by id)
    moved: List[tuple[Unit, Unit]] # Units with same id but different position
    changed_visible: List[tuple[Unit, Unit]]  # Units with same id, visibility changed
    changed_sensitive: List[tuple[Unit, Unit]] # Units with same id, sensitivity changed
    changed_name: List[tuple[Unit, Unit]]     # Units with same id, name changed


def diff_snapshots(old: Snapshot, new: Snapshot) -> DiffResult:
    """Diff two snapshots, matching units by id."""
    old_by_id = {u.id: u for u in old.units}
    new_by_id = {u.id: u for u in new.units}

    old_ids = set(old_by_id.keys())
    new_ids = set(new_by_id.keys())

    appeared = [new_by_id[i] for i in new_ids - old_ids]
    disappeared = [old_by_id[i] for i in old_ids - new_ids]

    common_ids = old_ids & new_ids

    moved = []
    changed_visible = []
    changed_sensitive = []
    changed_name = []

    for uid in common_ids:
        ou = old_by_id[uid]
        nu = new_by_id[uid]
        if ou.x != nu.x or ou.y != nu.y:
            moved.append((ou, nu))
        if ou.visible != nu.visible:
            changed_visible.append((ou, nu))
        if ou.sensitive != nu.sensitive:
            changed_sensitive.append((ou, nu))
        if ou.name != nu.name:
            changed_name.append((ou, nu))

    return DiffResult(
        appeared=appeared,
        disappeared=disappeared,
        moved=moved,
        changed_visible=changed_visible,
        changed_sensitive=changed_sensitive,
        changed_name=changed_name,
    )


def format_summary(summary: Summary) -> str:
    """Format a Summary as a human-readable string."""
    lines = []
    lines.append("=== Snapshot Summary ===")
    lines.append(f"Page: {summary.visible_page or 'unknown'}")
    lines.append(f"Focused unit id: {summary.focused_unit_id if summary.focused_unit_id is not None else 'none'}")
    lines.append(f"Total visible text length: {summary.total_visible_text_length}")
    lines.append("")
    lines.append("Counts by kind:")
    for kind, count in sorted(summary.counts_by_kind.items()):
        lines.append(f"  {kind}: {count}")
    lines.append("")
    lines.append(f"Unnamed interactive units ({len(summary.unnamed_interactive)}):")
    for u in summary.unnamed_interactive:
        lines.append(f"  id={u.id} kind={u.kind} role={u.role} bounds=({u.x},{u.y},{u.w},{u.h})")
    lines.append("")
    lines.append(f"Duplicate names ({len(summary.duplicate_names)}):")
    for name, units in sorted(summary.duplicate_names.items()):
        lines.append(f"  '{name}': {[u.id for u in units]}")
    lines.append("")
    lines.append(f"Hidden units ({len(summary.hidden_units)}):")
    for u in summary.hidden_units:
        lines.append(f"  id={u.id} kind={u.kind} name='{u.name}'")
    lines.append("")
    lines.append(f"Offscreen units ({len(summary.offscreen_units)}):")
    for u in summary.offscreen_units:
        lines.append(f"  id={u.id} kind={u.kind} name='{u.name}' bounds=({u.x},{u.y},{u.w},{u.h})")
    return "\n".join(lines)


def format_diff(diff: DiffResult) -> str:
    """Format a DiffResult as a human-readable string."""
    lines = []
    lines.append("=== Snapshot Diff ===")
    lines.append(f"Appeared ({len(diff.appeared)}):")
    for u in diff.appeared:
        lines.append(f"  id={u.id} kind={u.kind} name='{u.name}' bounds=({u.x},{u.y},{u.w},{u.h})")
    lines.append(f"Disappeared ({len(diff.disappeared)}):")
    for u in diff.disappeared:
        lines.append(f"  id={u.id} kind={u.kind} name='{u.name}'")
    lines.append(f"Moved ({len(diff.moved)}):")
    for ou, nu in diff.moved:
        lines.append(f"  id={ou.id} kind={ou.kind} name='{ou.name}' ({ou.x},{ou.y}) -> ({nu.x},{nu.y})")
    lines.append(f"Visibility changed ({len(diff.changed_visible)}):")
    for ou, nu in diff.changed_visible:
        lines.append(f"  id={ou.id} kind={ou.kind} name='{ou.name}' visible={ou.visible} -> {nu.visible}")
    lines.append(f"Sensitivity changed ({len(diff.changed_sensitive)}):")
    for ou, nu in diff.changed_sensitive:
        lines.append(f"  id={ou.id} kind={ou.kind} name='{ou.name}' sensitive={ou.sensitive} -> {nu.sensitive}")
    lines.append(f"Name changed ({len(diff.changed_name)}):")
    for ou, nu in diff.changed_name:
        lines.append(f"  id={ou.id} kind={ou.kind} '{ou.name}' -> '{nu.name}'")
    return "\n".join(lines)


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1:
        snap = load_snapshot(sys.argv[1])
        print(format_summary(summarize(snap)))
    else:
        print("Usage: python -m np_drive.snapshot <snapshot.json>")