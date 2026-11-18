//! ArgonSDK main: [UTBLLocalPlayer](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLLocalPlayer.h).

// Console command permissions.
define_pattern_resolver![UTBLLocalPlayer_Exec, {
    OTHER: [
        "75 ?? 45 84 ed 75 ?? 48 85 f6 74 ?? 40 38 be ?? 01 00 00",
        "75 18 40 38 7d d7 75 12 4d 85 f6 74 0d 41 38 be ?? 01 00 00"
    ],
    EGS: ["75 18 40 38 7d d7 75 12 4d 85 f6 74 0d 41 38 be ?? 01 00 00"],
}];
CREATE_PATCH!(UTBLLocalPlayer_Exec, BYTES, &[0xEB]);
