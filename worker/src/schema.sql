CREATE TABLE IF NOT EXISTS clips(
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          kind TEXT NOT NULL,
          content TEXT NOT NULL DEFAULT '',
          search TEXT NOT NULL DEFAULT '',
          files TEXT NOT NULL DEFAULT '[]',
          blob TEXT NULL,
          width INTEGER NOT NULL DEFAULT 0,
          height INTEGER NOT NULL DEFAULT 0,
          bytes INTEGER NOT NULL DEFAULT 0,
          hash TEXT NOT NULL,
          source TEXT NULL,
          created_ms INTEGER NOT NULL,
          updated_ms INTEGER NOT NULL,
          repeat_count INTEGER NOT NULL DEFAULT 1,
          pinned INTEGER NOT NULL DEFAULT 0,
          truncated INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS ix_clips_created ON clips(created_ms DESC);
        CREATE INDEX IF NOT EXISTS ix_clips_hash ON clips(hash);
        CREATE TABLE IF NOT EXISTS shots(
          id TEXT PRIMARY KEY,
          relative TEXT NOT NULL,
          thumb TEXT NOT NULL,
          width INTEGER NOT NULL DEFAULT 0,
          height INTEGER NOT NULL DEFAULT 0,
          bytes INTEGER NOT NULL DEFAULT 0,
          hash TEXT NOT NULL,
          source TEXT NOT NULL DEFAULT '',
          name TEXT NOT NULL DEFAULT '',
          search TEXT NOT NULL DEFAULT '',
          created_ms INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS ix_shots_hash ON shots(hash);
        CREATE INDEX IF NOT EXISTS ix_shots_created ON shots(created_ms DESC);
