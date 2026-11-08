# Console context gold revision

- Date: 2026-11-07 local (2026-11-08 UTC).
- Status: accepted; implementation uncommitted.
- Scope: default console module and function color.
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git); branch `log-standardization-finalization`; recording HEAD `90e6cd044d7eaa10148b613b6ac75d2ebc3fae10`; no exact tag; nearest tag `v1.0.0-RC3`. Previous implementation: `2c1649a027eadf0962c80567b25fe79bc297a601`, [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30).
- Source: supplied compacted conversation checkpoint, attributing the request to the user and implementation to the build agent. Source session and response IDs for this revision were not supplied; earlier conversation is unavailable. The three implementation and documentation files below were modified and uncommitted at recording.

## Decision and evidence

The user requested increasing the gold for module, `::`, and function from `#B49A66` to `#BE9D5C`. This supersedes only the specific color in [the earlier color decision](2026-11-07-console-function-separator-colors.md), not its layout or selection conclusions. The earlier value remains the historical committed baseline described in the [implementation milestone](2026-11-07-console-selection-format-implementation.md).

The uncommitted changes in `src/logging/log4rs/log4rs.default.yaml`, `src/logging/log4rs/palette.rs`, and `docs/logging.md` consistently specify the new color; palette test expectations also change to RGB `190, 157, 92`. The user preference is reported by the checkpoint; further rationale and other considered alternatives are unknown. Retaining the old shade was the existing baseline, not a documented separately evaluated alternative.

## Consequences and follow-up

The new shade is not in the recorded HEAD. The checkpoint reports no test run, rebuild, or preview for this revision; the earlier Proton results do not validate it, and native Windows remains untested. Review and test the change, rebuild and visually check readability, and commit it through the normal PR process if approved. Reconsider the shade if visual contrast, accessibility, or host rendering proves inadequate. The full discussion and source response identifier remain unavailable.

## Subsequent development (2026-11-07)

A later supplied checkpoint reports that the user requested a relaunch and the preview initialized; visual approval was not reported. The external `unchained-plugin-log-saturated-gold-preview/saturatedgold/launch.json` identifies a Steam preview DLL with SHA-256 `3426f27f9561e496e57a090f7418c203071af61d9004a544299486f45e96910f`, matching the available DLL. The associated profile's `unchained.log` shows plugin activity. These artifacts support a preview launch but do not independently establish its source tree or visual appearance. The earlier statement about no preview describes the evidence available at that recording; the later checkpoint does not identify a test run. Review the color visually and test before committing. Source session and response IDs for this later checkpoint were not supplied.
