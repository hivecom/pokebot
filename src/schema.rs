// @generated automatically by Diesel CLI.

diesel::table! {
    use crate::sqlite_mapping::*;

    albums (id) {
        id -> Integer,
        title -> Text,
        artist_id -> Nullable<Integer>,
        cover -> Nullable<Text>,
        created_at -> Integer,
        created_by -> Text,
    }
}

diesel::table! {
    use crate::sqlite_mapping::*;

    artists (id) {
        id -> Integer,
        name -> Text,
        created_at -> Integer,
        created_by -> Text,
    }
}

diesel::table! {
    use crate::sqlite_mapping::*;

    audio_files (id) {
        id -> Integer,
        file_name -> Text,
        file_path -> Text,
        duration -> Integer,
        created_at -> Integer,
        created_by -> Text,
    }
}

diesel::table! {
    use crate::sqlite_mapping::*;

    favourites (id) {
        id -> Integer,
        uid -> Text,
        song_id -> Integer,
        created_at -> Integer,
    }
}

diesel::table! {
    use crate::sqlite_mapping::*;

    playlist_songs (id) {
        id -> Text,
        song_id -> Integer,
        playlist_id -> Integer,
        created_at -> Integer,
    }
}

diesel::table! {
    use crate::sqlite_mapping::*;

    playlists (id) {
        id -> Integer,
        name -> Text,
        created_at -> Integer,
        created_by -> Text,
    }
}

diesel::table! {
    use crate::sqlite_mapping::*;

    songs (id) {
        id -> Integer,
        track -> Nullable<Integer>,
        title -> Text,
        artist_id -> Integer,
        album_id -> Nullable<Integer>,
        file_id -> Integer,
        created_at -> Integer,
        created_by -> Text,
        disc_number -> Nullable<Integer>,
    }
}

diesel::table! {
    use crate::sqlite_mapping::*;

    tokens (token) {
        token -> Text,
        uid -> Text,
        created_at -> Integer,
    }
}

diesel::joinable!(albums -> artists (artist_id));
diesel::joinable!(favourites -> songs (song_id));
diesel::joinable!(playlist_songs -> playlists (playlist_id));
diesel::joinable!(playlist_songs -> songs (song_id));
diesel::joinable!(songs -> albums (album_id));
diesel::joinable!(songs -> artists (artist_id));
diesel::joinable!(songs -> audio_files (file_id));

diesel::allow_tables_to_appear_in_same_query!(
    albums,
    artists,
    audio_files,
    favourites,
    playlist_songs,
    playlists,
    songs,
    tokens,
);
