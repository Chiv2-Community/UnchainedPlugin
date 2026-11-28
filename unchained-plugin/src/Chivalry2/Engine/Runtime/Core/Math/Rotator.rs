//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Math/Rotator.h>.

#[unchained_macros_core::runtime_bindings]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct FRotator {
    /// Rotation around the right axis (Look up/down), 0=Horizontal, +Up, -Down.
    pub pitch: f32, // 0x0
    /// Rotation around the up axis (Turn left/right), 0=Forward, +Right, -Left.
    pub yaw: f32, // 0x4
    /// Rotation around the forward axis (Tilt left/right), 0=No tilt, +Clockwise, -CCW.
    pub roll: f32, // 0x8
}

impl FRotator {
    pub const ZERO: Self = Self {
        pitch: 0.0,
        yaw: 0.0,
        roll: 0.0,
    };

    pub fn new(pitch: f32, yaw: f32, roll: f32) -> Self {
        Self { pitch, yaw, roll }
    }

    /// Returns a vector pointing in the direction of this rotation
    pub fn to_vector(&self) -> [f32; 3] {
        let p = self.pitch.to_radians();
        let y = self.yaw.to_radians();

        let cp = p.cos();
        let sp = p.sin();
        let cy = y.cos();
        let sy = y.sin();

        [cp * cy, cp * sy, sp]
    }
}
