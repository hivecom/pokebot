CREATE TABLE tokens (
    "token" TEXT PRIMARY KEY NOT NULL,
    "uid" TEXT NOT NULL,
    "created_at" INTEGER NOT NULL-- unix ts
) STRICT;

CREATE TABLE audio_files (
    "id" INTEGER PRIMARY KEY NOT NULL,
    "file_name" TEXT NOT NULL,
    "file_path" TEXT NOT NULL UNIQUE,
    "duration" INTEGER NOT NULL,

    "created_at" INTEGER NOT NULL, -- unix ts
    "created_by" TEXT NOT NULL
) STRICT;

CREATE TABLE artists (
    "id" INTEGER PRIMARY KEY NOT NULL,
    "name" TEXT NOT NULL UNIQUE,
    "created_at" INTEGER NOT NULL,
    "created_by" TEXT NOT NULL
) STRICT;

CREATE TABLE albums (
    "id" INTEGER PRIMARY KEY NOT NULL,
    "title" TEXT NOT NULL UNIQUE,
    "artist_id" INTEGER NULL,
    "cover" TEXT NULL,

    "created_at" INTEGER NOT NULL, -- unix ts
    "created_by" TEXT NOT NULL,

    FOREIGN KEY (artist_id)
    REFERENCES artists (id)
    ON DELETE SET NULL
) STRICT;

CREATE TABLE songs (
    "id" INTEGER PRIMARY KEY NOT NULL,
    "track" INTEGER NULL,
    "title" TEXT NOT NULL,
    "artist_id" INTEGER NOT NULL,
    "album_id" INTEGER NULL,

    "file_id" INTEGER NOT NULL UNIQUE,

    "created_at" INTEGER NOT NULL, -- unix ts
    "created_by" TEXT NOT NULL,

    FOREIGN KEY (artist_id)
    REFERENCES artists (id),

    FOREIGN KEY (album_id)
    REFERENCES albums (id)
    ON DELETE SET NULL,

    FOREIGN KEY (file_id)
    REFERENCES audio_files (id)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE favourites (
    "id" INTEGER PRIMARY KEY NOT NULL,
    "uid" TEXT NOT NULL,
    "song_id" INTEGER NOT NULL,

    "created_at" INTEGER NOT NULL, -- unix ts

    UNIQUE (uid, song_id),

    FOREIGN KEY (song_id)
    REFERENCES songs (id)
    ON DELETE CASCADE
) STRICT;

CREATE TABLE playlists (
    "id" INTEGER PRIMARY KEY NOT NULL,
    "name" TEXT NOT NULL,

    "created_at" INTEGER NOT NULL, -- unix ts
    "created_by" TEXT NOT NULL
) STRICT;

CREATE TABLE playlist_songs (
    "id" TEXT PRIMARY KEY NOT NULL,
    "song_id" INTEGER NOT NULL,
    "playlist_id" INTEGER NOT NULL,

    "created_at" INTEGER NOT NULL, -- unix ts

    UNIQUE (song_id, playlist_id),

    FOREIGN KEY (song_id)
    REFERENCES songs (id)
    ON DELETE CASCADE,

    FOREIGN KEY (playlist_id)
    REFERENCES playlists (id)
    ON DELETE CASCADE
) STRICT;

-- vim: expandtab sw=4 ts=4
