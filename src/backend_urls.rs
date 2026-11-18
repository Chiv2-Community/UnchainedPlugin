//! URLs used by the game's native HTTP transport.

pub fn tbio_base_url(base_url: &str) -> String {
    format!("{}/api/tbio", base_url.trim_end_matches('/'))
}

pub fn playfab_matchmake_url(base_url: &str) -> String {
    format!("{}/api/playfab/Client/Matchmake", base_url.trim_end_matches('/'))
}
