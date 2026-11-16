//! Replace ListPlayers and send one corrected table to the console and clipboard.
use super::{gameplay::object_inherits_from, messages::o_FText_ToString};
use crate::{
    game::{
        chivalry2::{ATBLPlayerState, PlayerFlags},
        engine::{UIpConnection, UIpNetDriver},
    },
    ue::{APlayerController, TCHAR},
};
use tabled::{
    builder::Builder,
    settings::{Alignment, Padding, Style},
};

define_pattern_resolver!(ListPlayers, [
    "48 8B C4 55 48 8D 68 A1 48 81 EC F0 00 00 00 48 89 58 F0 48 89 70 E8 48 89 78 E0 33 FF 4C 89 60 D8 4C 89 70 C8 4C 8B F1 48 8B 01 89 7D 67 FF 90 50 01 00 00"
]);

define_pattern_resolver!(
    ListPlayersClipboard,
    Call,
    ["49 8B CC E8 | ?? ?? ?? ?? 48 8B 4D D7 4C 8B B4 24 C0 00 00 00 4C 8B A4 24 D0 00 00 00"]
);
CREATE_HOOK!(ListPlayersClipboard, CALLED, i32, (text: *const TCHAR), {});

CREATE_HOOK!(ListPlayers, ACTIVE, NONE, (), (controller: *mut APlayerController), {
    if controller.is_null() { return; }
    let output = player_output();
    super::messages::print_console_output(controller, &output);
    let utf16: Vec<u16> = output.encode_utf16().chain(Some(0)).collect();
    if let Err(error) = CALL_ORIGINAL_SAFE!(ListPlayersClipboard(utf16.as_ptr())) {
        serror!("ListPlayers clipboard output failed: {error}");
    }
});

unsafe fn player_output() -> String {
    let world = crate::globals().world().and_then(|world| world.as_ref());
    let state = world.and_then(|world| world.game_state.as_ref());
    let name = match state {
        Some(state) => {
            // Native FText::ToString rebuilds text before consulting its data vtable.
            CALL_ORIGINAL_SAFE!(FText_ToString(&state.server_name))
                .ok()
                .and_then(|text| text.as_ref())
                .map(ToString::to_string)
                .unwrap_or_default()
        }
        None => "no server name".into(),
    };
    let address = world
        .and_then(|world| {
            let driver = world.net_driver.as_ref()?;
            if !object_inherits_from(world.net_driver.cast(), "IpNetDriver") {
                return None;
            }
            let address = if driver.server_connection.is_null() {
                (*world.net_driver.cast::<UIpNetDriver>()).local_addr
            } else {
                if !object_inherits_from(driver.server_connection, "IpConnection") {
                    return None;
                }
                (*driver.server_connection.cast::<UIpConnection>()).remote_addr
            };
            Some(address.as_ref()?.to_native_string(true))
        })
        .unwrap_or_default();
    let rows: Vec<Row> = state
        .map(|state| state.player_array.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|player| player.as_ref())
        .filter(|player| {
            object_inherits_from(
                (*player as *const ATBLPlayerState).cast_mut().cast(),
                "TBLPlayerState",
            )
        })
        .filter(|player| !player.base.player_flags.contains(PlayerFlags::IS_A_BOT))
        .map(|player| {
            [
                player.base.get_player_name(),
                player
                    .playfab_oss_unique_id
                    .to_native_string()
                    .unwrap_or_default(),
                player.base.unique_id.to_native_string().unwrap_or_default(),
                player.player_score.to_string(),
                player.kills.to_string(),
                player.deaths.to_string(),
                format!("{} ms", u32::from(player.base.ping) * 4),
            ]
        })
        .collect();
    format!("ServerName - {name} {address}\n{}", render(&rows))
}

const HEADERS: [&str; 7] = [
    "Name",
    "PlayFabPlayerId",
    "UniqueNetId",
    "Score",
    "Kills",
    "Deaths",
    "Ping",
];

type Row = [String; HEADERS.len()];

// A cell must stay on one line; names are data, never format strings.
fn cell(value: &str) -> String {
    if value.is_empty() {
        return "NULL".into();
    }
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn render(rows: &[Row]) -> String {
    let mut builder = Builder::default();
    builder.push_record(HEADERS);
    for row in rows {
        builder.push_record(row.iter().map(|value| cell(value)));
    }
    let mut table = builder.build();
    table.with((Style::blank(), Alignment::left(), Padding::new(0, 1, 0, 0)));
    format!("{table}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(values: [&str; 7]) -> Row {
        values.map(str::to_owned)
    }

    fn assert_cells(output: &str, expected: &[Row]) {
        let expected: Vec<Row> = std::iter::once(HEADERS.map(str::to_owned))
            .chain(expected.iter().cloned())
            .collect();
        let lines: Vec<_> = output.lines().collect();
        assert_eq!(lines.len(), expected.len());
        for (line, row) in lines.iter().zip(&expected) {
            let mut cursor = 0;
            for value in row {
                assert!(!value.is_empty(), "every cell must contain a value");
                let start = cursor
                    + line[cursor..]
                        .find(value)
                        .expect("cell data must be present in column order");
                cursor = start + value.len();
            }
        }
    }

    #[test]
    fn empty_list_keeps_headers() {
        assert_cells(&render(&[]), &[]);
    }

    #[test]
    fn unicode_percent_and_duplicate_names_are_preserved() {
        let rows = [
            row(["玩家 %s", "epic", "id", "501", "23", "4", "96 ms"]),
            row(["玩家 %s", "NULL", "NULL", "0", "0", "0", "0 ms"]),
            row(["e\u{301}", "steam", "other", "1", "2", "3", "4 ms"]),
        ];
        assert_cells(&render(&rows), &rows);
    }

    #[test]
    fn controls_cannot_inject_rows_or_break_spacing() {
        let output = render(&[row([
            "A\nB\rC\tD\0",
            "NULL",
            "NULL",
            "-1",
            "2",
            "3",
            "8 ms",
        ])]);
        assert_cells(
            &output,
            &[row(["A B C D ", "NULL", "NULL", "-1", "2", "3", "8 ms"])],
        );
    }

    #[test]
    fn empty_cells_have_explicit_values() {
        let rows = [row(["", "", "", "0", "0", "0", "0 ms"])];
        let expected = [row(["NULL", "NULL", "NULL", "0", "0", "0", "0 ms"])];
        assert_cells(&render(&rows), &expected);
    }
}
