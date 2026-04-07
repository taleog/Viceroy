#!/usr/bin/env python3
"""
Generate a large clipboard history dataset for Viceroy's sqlite DB.
This script writes directly to the clipboard_history table.

Usage:
  python3 generate_clipboard_dataset.py --db ~/.local/share/viceroy/clipboard.db --count 5000 --image-pct 10

Do not run this on a production DB unless you know what you're doing. It is intended for profiling on local/dev clones.
"""
import argparse
import sqlite3
import time
import base64
from random import randint, choice, random

SAMPLE_PNG_B64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR4nGNgYAAAAAMAASsJTYQAAAAASUVORK5CYII="

def make_text(i):
    return f"generated clipboard text item {i} - {int(time.time())}"

def make_image_b64():
    return SAMPLE_PNG_B64

if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--db', required=True)
    p.add_argument('--count', type=int, default=5000)
    p.add_argument('--image-pct', type=float, default=5.0, help='percentage of entries that are images')
    args = p.parse_args()

    conn = sqlite3.connect(args.db)
    cur = conn.cursor()

    # Ensure the table exists (basic sanity)
    cur.execute("""
    CREATE TABLE IF NOT EXISTS clipboard_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        content TEXT NOT NULL,
        content_type TEXT NOT NULL,
        app_name TEXT,
        timestamp INTEGER NOT NULL,
        custom_name TEXT,
        is_favorite INTEGER DEFAULT 0,
        is_pinned INTEGER DEFAULT 0,
        image_width INTEGER,
        image_height INTEGER,
        sync_id TEXT,
        source_device_id TEXT,
        source_device_name TEXT,
        updated_at INTEGER,
        deleted_at INTEGER
    );
    """)
    conn.commit()

    for i in range(1, args.count + 1):
        ts = int(time.time()) - randint(0, 60*60*24*30)
        if random() * 100 < args.image_pct:
            content = make_image_b64()
            content_type = 'image'
            image_width = 16
            image_height = 16
        else:
            content = make_text(i)
            content_type = 'text'
            image_width = None
            image_height = None
        cur.execute(
            "INSERT INTO clipboard_history (content, content_type, app_name, timestamp, image_width, image_height) VALUES (?, ?, ?, ?, ?, ?)",
            (content, content_type, choice(['Terminal','Safari','Notes','Mail','VSCode']), ts, image_width, image_height)
        )
        if i % 500 == 0:
            conn.commit()
            print(f"Inserted {i}/{args.count}")
    conn.commit()
    print('Done')
