table! {
    history (id) {
        id -> Integer,
        user_id -> Nullable<Integer>,
        song_id -> Integer,
        matched_at -> Timestamp,
    }
}

table! {
    logs (id) {
        id -> Integer,
        login -> Text,
        logging_time -> Timestamp,
        logging_succession -> Bool,
        ip_addr -> Text,
        user_agent -> Text,
    }
}

table! {
    songs (id) {
        id -> Integer,
        artist -> Text,
        title -> Text,
        genre -> Text,
        url -> Text,
        featured -> Bool,
    }
}

table! {
    users (id) {
        id -> Integer,
        login -> Text,
        hash -> Text,
        role -> Text,
        active -> Bool,
    }
}

joinable!(history -> songs (song_id));
joinable!(history -> users (user_id));

allow_tables_to_appear_in_same_query!(history, logs, songs, users,);
