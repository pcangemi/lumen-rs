//! Native macOS input regression. Opens a temporary window and injects local AppKit
//! events into that window only; no accessibility permission or global input is used.
#[cfg(target_os = "macos")]
#[path = "../src/bin/viewer/input.rs"]
mod input;

#[cfg(target_os = "macos")]
#[allow(
    unexpected_cfgs,
    reason = "objc 0.2 macros contain legacy cargo-clippy feature checks"
)]
fn main() -> anyhow::Result<()> {
    use glam::Vec3;
    use minifb::{Window, WindowOptions};
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};
    use std::{ffi::c_void, thread, time::Duration};

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, down: bool) -> *mut c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *const c_void);
    }

    objc::rc::autoreleasepool(|| -> anyhow::Result<()> {
        let mut window = Window::new("Lumen input regression", 480, 240, WindowOptions::default())?;
        window.set_target_fps(0);
        let pixels = vec![0x18232c; 480 * 240];
        // SAFETY: The test runs on the main thread, owns this window, and dispatches
        // events only to its NSWindow. Core Graphics events are released after use.
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let native = window.get_window_handle().cast::<Object>();
            let _: () = msg_send![app, activateIgnoringOtherApps: objc::runtime::YES];
            let _: () = msg_send![native, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
            for _ in 0..40 {
                window.update_with_buffer(&pixels, 480, 240)?;
                if input::focused(&mut window) {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            anyhow::ensure!(
                input::focused(&mut window),
                "Test window did not receive focus"
            );
            println!(
                "AppKit focused=true, minifb is_active={}",
                window.is_active()
            );

            let key_event = |key, down| {
                let cg = CGEventCreateKeyboardEvent(std::ptr::null(), key, down);
                assert!(!cg.is_null());
                let event: *mut Object = msg_send![class!(NSEvent), eventWithCGEvent: cg];
                let _: () = msg_send![native, sendEvent: event];
                CFRelease(cg);
            };
            for (key, name, movement) in [
                (13, "W", Vec3::Z),
                (0, "A", -Vec3::X),
                (1, "S", -Vec3::Z),
                (2, "D", Vec3::X),
                (126, "Up", Vec3::Z),
            ] {
                key_event(key, true);
                window.update();
                let state = input::Input::read(&mut window);
                assert_eq!(state.movement, movement, "{name} did not move");
                assert!(!state.save_image && !state.toggle_denoise);
                assert!(state.active && state.speed > 0.0);
                key_event(key, false);
                window.update();
                assert_eq!(input::Input::read(&mut window).movement, Vec3::ZERO);
                println!("{name}: movement without mouse, no image shortcut");
            }
            key_event(3, true); // F
            window.update();
            assert!(input::Input::read(&mut window).toggle_flight);
            key_event(3, false);
            window.update();
            key_event(48, true); // Tab
            window.update();
            assert!(input::Input::read(&mut window).toggle_denoise);
            key_event(48, false);
            window.update();
            println!("F and Tab: mode and display shortcuts received");

            // The minifb NSView's mouse handlers only use the event type.
            let view: *mut Object = msg_send![native, contentView];
            let _: () = msg_send![view, rightMouseDown: std::ptr::null_mut::<Object>()];
            assert!(input::Input::read(&mut window).looking);
            let _: () = msg_send![view, rightMouseUp: std::ptr::null_mut::<Object>()];
            assert!(!input::Input::read(&mut window).looking);
            println!("Right mouse: look input enabled and released");

            let _: () = msg_send![native, resignKeyWindow];
            assert!(!input::focused(&mut window));
            key_event(13, true);
            window.update();
            let state = input::Input::read(&mut window);
            assert!(!state.active && !state.looking);
            assert_eq!(state.movement, Vec3::ZERO);
            key_event(13, false);
            println!("Unfocused window: movement rejected");
        }
        Ok(())
    })
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This native input regression requires macOS.");
}
