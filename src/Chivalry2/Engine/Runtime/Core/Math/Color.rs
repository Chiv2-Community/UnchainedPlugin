//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Math/Color.h>.

#[unchained_macros_core::runtime_bindings]
#[derive(Debug, Default)]
#[repr(C)]
pub struct FLinearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl FLinearColor {
    /// Rust sRGB8 adapter with caller-supplied linear alpha.
    /// Uses the existing runtime formula, not UE's native sRGB lookup table;
    /// bitwise equality with FLinearColor::FromSRGBColor is not implied.
    pub(crate) fn from_srgb8(rgb: [u8; 3], alpha: f32) -> Self {
        let [r, g, b] = rgb.map(|component| {
            let srgb = component as f32 / 255.0;
            if srgb <= 0.04045 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            }
        });
        Self { r, g, b, a: alpha }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb8_initializes_channels_and_preserves_linear_alpha() {
        let color = FLinearColor::from_srgb8([255, 128, 0], 0.25);
        assert_eq!([color.r, color.b, color.a], [1.0, 0.0, 0.25]);
        assert!((color.g - 0.2158605).abs() < 0.000001);
    }

    #[test]
    fn srgb8_preserves_runtime_formula_for_every_component() {
        for component in 0..=255u8 {
            let srgb = component as f32 / 255.0;
            let expected = if srgb <= 0.04045 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            };
            let color = FLinearColor::from_srgb8([component; 3], 1.0);
            assert_eq!(
                [color.r.to_bits(), color.g.to_bits(), color.b.to_bits()],
                [expected.to_bits(); 3]
            );
            assert_eq!(color.a, 1.0);
        }
    }
}
