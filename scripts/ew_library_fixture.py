"""Synthetic EasyWorship 6-8 library for the native import checks
(scripts/ew-import-x11.py, scripts/ew-import-windows.py; M4-07).

Original text only. The schema mirrors src/import/ew_fixture.rs and is the
UNOBSERVED best guess in docs/import.md, "EasyWorship library": never a
copy of a real library. Standard library only, so it runs on Linux and
Windows.
"""

from __future__ import annotations

import hashlib
import sqlite3
import struct
from pathlib import Path

IMPORTED = {"Harbor Morning", "anchor of grace", "Kasih Setia"}
# Preview rows: title order, case ignored; the song without words is named
# by its rowid.
ROWS = [
    "anchor of grace",
    "Harbor Morning",
    "Harbor Morning (copy)",
    "Kasih Setia",
    "Songs.db #4",
]

HARBOR_SECTIONS = [
    "Verse 1",
    "Light on the water\nwakes the town",
    "",
    "Boats lean home",
    "Chorus",
    "Sing, harbor, sing",
]
KASIH_SECTION = ["Verse 1", "Kasih-Mu \u2014 setia\nselamanya"]
SUMMARY = "Ready: 4 · Not importable: 1 · Duplicates: 1"
REIMPORT_DUPLICATES = "Duplicates: 4"

SONGS_SCHEMA = """
CREATE TABLE song (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    song_item_uid TEXT, song_rev_uid TEXT, song_uid TEXT,
    title TEXT COLLATE UTF8_U_CI, author TEXT COLLATE UTF8_U_CI,
    copyright TEXT, administrator TEXT, description TEXT, tags TEXT,
    reference_number TEXT, vendor_id INTEGER, presentation_id INTEGER,
    layout_revision INTEGER, revision INTEGER);
CREATE INDEX song_title ON song (title COLLATE UTF8_U_CI);
"""
WORDS_SCHEMA = """
CREATE TABLE word (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    song_id INTEGER, words TEXT, slide_uids TEXT,
    slide_layout_revisions TEXT, slide_revisions TEXT);
CREATE INDEX word_song_id ON word (song_id);
"""


def ew8_words(sections: list[tuple[str, list[str]]]) -> str:
    """Words in the observed EW8 dialect (docs/import.md, "Observed format")."""

    def para(text: str, flags: str = "", style: int = 101) -> str:
        return (
            f"{{\\pard{flags}\\qc\\qdef\\sdewparatemplatestyle{style}\\plain"
            f"\\sdewtemplatestyle{style}\\fs156\\sdfsauto {text}\\par}}\r\n"
        )

    marker = para("", "\\sdslidemarker")
    out = "{\\rtf1\\ansi\\deff0\\sdeasyworship2\r\n{\\fonttbl{\\f0 Tahoma;}}\r\n{\\colortbl ;}\r\n"
    for number, (label, slides) in enumerate(sections):
        if number:
            out += marker
        out += para(label, "\\sdparawysiwghidden\\sdlistlevel-1", 102)
        for index, slide in enumerate(slides):
            if index:
                out += marker
            out += "".join(para(line) for line in slide.split("\n"))
    return out + "}"


def ci(a: str, b: str) -> int:
    a, b = a.lower(), b.lower()
    return (a > b) - (a < b)


def write_ew_library(data: Path) -> None:
    """Songs.db and SongWords.db with five song rows in `data` (created)."""
    data.mkdir(parents=True)
    harbor = ew8_words(
        [
            ("Verse 1", ["Light on the water\nwakes the town", "Boats lean home"]),
            ("Chorus", ["Sing, harbor, sing"]),
        ]
    )
    anchor = (
        "{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fcharset0 Arial;}}\\f0 Verse 1\\par "
        "Hold fast the line\\par\\par Chorus\\par Grace holds the rope\\par}"
    )
    kasih = ew8_words([("Verse 1", ["Kasih-Mu \\u8212? setia\\line selamanya"])])
    # (uid, title, author, copyright, administrator, vendor_id, description, words)
    songs = [
        ("{H}", "Harbor Morning", "Rina Kartika", "2026 Sela Test Fixtures",
         "Fixture Music Admin", 7654321, "Opening song", harbor),
        ("{G}", "anchor of grace", None, None, None, "CCLI-12", None, anchor),
        ("{K}", "Kasih Setia", None, None, None, None, None, kasih),
        ("{L}", "Lost Words", None, None, None, None, None, None),
        ("{H}", "Harbor Morning (copy)", None, None, None, None, None, harbor),
    ]  # fmt: skip
    songs_db = sqlite3.connect(data / "Songs.db")
    words_db = sqlite3.connect(data / "SongWords.db")
    for db in (songs_db, words_db):
        db.create_collation("UTF8_U_CI", ci)
    songs_db.executescript(SONGS_SCHEMA)
    words_db.executescript(WORDS_SCHEMA)
    for uid, title, author, copyright_, admin, vendor, description, words in songs:
        cursor = songs_db.execute(
            "INSERT INTO song (song_uid, title, author, copyright, administrator,"
            " vendor_id, description, revision) VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
            (uid, title, author, copyright_, admin, vendor, description),
        )
        if words is not None:
            words_db.execute(
                "INSERT INTO word (song_id, words) VALUES (?, ?)",
                (cursor.lastrowid, words),
            )
    for db in (songs_db, words_db):
        db.commit()
        db.close()


def fingerprint(root: Path) -> list[tuple[str, str, int, int]]:
    """(relative path, SHA-256, size, mtime_ns) of every file under root."""
    out = []
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        stat = path.stat()
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        out.append(
            (str(path.relative_to(root)), digest, stat.st_size, stat.st_mtime_ns)
        )
    return out


def payload_fields(data: bytes) -> list[str]:
    """Sela song payload fields (title, metadata, then label/lyrics)."""
    fields = []
    while data:
        length = struct.unpack("<I", data[:4])[0]
        fields.append(data[4 : 4 + length].decode("utf-8"))
        data = data[4 + length :]
    return fields


def library_song(library: Path, title: str) -> tuple[list[str], tuple[str, str]] | None:
    """Payload fields and (administrator, song number) of the head revision
    of the Sela library song titled `title`, read-only."""
    uri = Path(library).resolve().as_uri() + "?mode=ro"
    with sqlite3.connect(uri, uri=True, timeout=2) as db:
        rows = db.execute(
            "SELECT r.payload, m.administrator, m.song_number FROM songs s "
            "JOIN song_revisions r ON r.id=s.id AND r.revision=s.head "
            "LEFT JOIN song_metadata m ON m.song=s.id AND m.revision=s.head "
            "WHERE s.deleted=0"
        ).fetchall()
    for payload, admin, number in rows:
        fields = payload_fields(payload)
        if fields[0] == title:
            return fields, (admin or "", number or "")
    return None


def check_harbor(fields: list[str], admin: str, number: str) -> list[str]:
    """What is wrong with the imported Harbor Morning, if anything."""
    problems = []
    if fields[1:3] != ["Rina Kartika", "2026 Sela Test Fixtures"]:
        problems.append(f"Harbor Morning authors/copyright {fields[1:3]}")
    if (admin, number) != ("Fixture Music Admin", "7654321"):
        problems.append(f"Harbor Morning administrator/CCLI {(admin, number)}")
    if fields[4:] != HARBOR_SECTIONS:
        problems.append(f"Harbor Morning sections {fields[4:]}")
    return problems
