//! Focus-aware navigation controls, with movement separate from image shortcuts.
use glam::Vec3;
use minifb::{Key, KeyRepeat, MouseButton, Window};

pub struct Input {
    pub active: bool,
    pub movement: Vec3,
    pub looking: bool,
    pub speed: f32,
    pub toggle_flight: bool,
    pub save_image: bool,
    pub toggle_denoise: bool,
}

impl Input {
    pub fn read(window: &mut Window) -> Self {
        let active = focused(window);
        Self::from_keys(
            active,
            |key| window.is_key_down(key),
            |key| window.is_key_pressed(key, KeyRepeat::No),
            window.get_mouse_down(MouseButton::Right),
        )
    }

    fn from_keys(
        active: bool,
        down: impl Fn(Key) -> bool,
        pressed: impl Fn(Key) -> bool,
        right_mouse: bool,
    ) -> Self {
        let shortcut = [
            Key::LeftCtrl,
            Key::RightCtrl,
            Key::LeftSuper,
            Key::RightSuper,
        ]
        .into_iter()
        .any(&down);
        let movement = if active && !shortcut {
            let x = (down(Key::Right) || down(Key::D)) as i32
                - (down(Key::Left) || down(Key::A) || down(Key::Q)) as i32;
            let z = (down(Key::Up) || down(Key::W) || down(Key::Z)) as i32
                - (down(Key::Down) || down(Key::S)) as i32;
            let y = down(Key::PageUp) as i32 - down(Key::PageDown) as i32;
            Vec3::new(x as f32, y as f32, z as f32)
        } else {
            Vec3::ZERO
        };
        Self {
            active,
            movement,
            looking: active && right_mouse,
            speed: if down(Key::LeftShift) || down(Key::RightShift) {
                3.0
            } else {
                1.2
            },
            toggle_flight: active && !shortcut && pressed(Key::F),
            save_image: active && shortcut && pressed(Key::S),
            toggle_denoise: active && !shortcut && pressed(Key::Tab),
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn focused(window: &mut Window) -> bool {
    window.is_active()
}

#[cfg(target_os = "macos")]
#[allow(
    unexpected_cfgs,
    reason = "objc 0.2 macros contain legacy cargo-clippy feature checks"
)]
pub fn focused(window: &mut Window) -> bool {
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};
    // minifb 0.28 inverts its native macOS focus flag. Query AppKit directly;
    // also reject an inactive application even if its window remains key.
    // SAFETY: Called on the UI thread while the minifb-owned NSWindow is alive.
    // Both selectors return AppKit BOOL and do not transfer object ownership.
    unsafe {
        let native = window.get_window_handle().cast::<Object>();
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let active: objc::runtime::BOOL = msg_send![app, isActive];
        let key: objc::runtime::BOOL = msg_send![native, isKeyWindow];
        active == objc::runtime::YES && key == objc::runtime::YES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(active: bool, keys: &[Key]) -> Input {
        Input::from_keys(
            active,
            |key| keys.contains(&key),
            |key| keys.contains(&key),
            false,
        )
    }

    #[test]
    fn movement_does_not_require_mouse_or_trigger_image_shortcuts() {
        for (key, expected) in [
            (Key::W, Vec3::Z),
            (Key::Z, Vec3::Z),
            (Key::Up, Vec3::Z),
            (Key::A, -Vec3::X),
            (Key::Q, -Vec3::X),
            (Key::Left, -Vec3::X),
            (Key::S, -Vec3::Z),
            (Key::Down, -Vec3::Z),
            (Key::D, Vec3::X),
            (Key::Right, Vec3::X),
        ] {
            let value = input(true, &[key]);
            assert_eq!(value.movement, expected, "{key:?}");
            assert!(!value.save_image && !value.toggle_denoise);
        }
        assert!(input(true, &[Key::F]).toggle_flight);
        assert!(input(true, &[Key::Tab]).toggle_denoise);
    }

    #[test]
    fn save_modifiers_and_inactive_windows_never_move_the_camera() {
        for modifier in [
            Key::LeftCtrl,
            Key::RightCtrl,
            Key::LeftSuper,
            Key::RightSuper,
        ] {
            let value = input(true, &[modifier, Key::S]);
            assert!(value.save_image);
            assert_eq!(value.movement, Vec3::ZERO);
        }
        let inactive = Input::from_keys(false, |_| true, |_| true, true);
        assert_eq!(inactive.movement, Vec3::ZERO);
        assert!(!inactive.looking && !inactive.toggle_flight && !inactive.save_image);
    }
}
