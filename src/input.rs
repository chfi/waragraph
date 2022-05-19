#![allow(non_upper_case_globals)]

use bstr::ByteSlice;
use rustc_hash::FxHashMap;
use winit::event::ModifiersState;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use parking_lot::RwLock;

use rhai::plugin::*;

use rhai::ImmutableString;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use crossbeam::atomic::AtomicCell;

use lazy_static::lazy_static;

lazy_static! {
    static ref MOUSE_POS: AtomicCell<(f64, f64)> = AtomicCell::new((0.0, 0.0));
    static ref MOD_KEYS: AtomicCell<ModifiersState> =
        AtomicCell::new(ModifiersState::empty());
}

pub fn set_mouse_pos(x: f64, y: f64) {
    MOUSE_POS.store((x, y));
}

pub fn get_mouse_pos() -> (f64, f64) {
    MOUSE_POS.load()
}

pub fn set_modifiers(state: ModifiersState) {
    MOD_KEYS.store(state);
}

pub fn create_mouse_module() -> rhai::Module {
    let mut module: rhai::Module = rhai::exported_module!(mouse);

    module.set_native_fn("get_pos", || {
        let (x, y) = MOUSE_POS.load();
        let mut pos = rhai::Map::default();
        pos.insert("x".into(), rhai::Dynamic::from_float(x as f32));
        pos.insert("y".into(), rhai::Dynamic::from_float(y as f32));
        Ok(pos)
    });

    module
}

pub fn create_key_module() -> rhai::Module {
    let mut module: rhai::Module = rhai::exported_module!(key);

    module.set_native_fn("get_modifiers", || {
        let mods = MOD_KEYS.load();
        let mut map = rhai::Map::default();
        map.insert("ctrl".into(), mods.ctrl().into());
        map.insert("shift".into(), mods.shift().into());
        map.insert("alt".into(), mods.alt().into());
        map.insert("logo".into(), mods.logo().into());
        Ok(map)
    });

    add_keys(&mut module);

    module
}

#[export_module]
pub mod mouse {
    use winit::event;

    pub type Button = event::MouseButton;

    pub const Left: Button = event::MouseButton::Left;
    pub const Right: Button = event::MouseButton::Right;
    pub const Middle: Button = event::MouseButton::Middle;
}

#[export_module]
pub mod key {

    use winit::event;

    pub type Key = event::VirtualKeyCode;
}

fn add_keys(module: &mut rhai::Module) {
    macro_rules! impl_keys {
        ( $( $key:ident ),* ) => {
            $(module.set_var(
                stringify!($key),
                rhai::Dynamic::from(winit::event::VirtualKeyCode::$key),
            );)*
            // $(pub const $key: Key = event::VirtualKeyCode::$key;)*
        };
    }

    impl_keys![
        A,
        B,
        C,
        D,
        E,
        F,
        G,
        H,
        I,
        J,
        K,
        L,
        M,
        N,
        O,
        P,
        Q,
        R,
        S,
        T,
        U,
        V,
        W,
        X,
        Y,
        Z,
        Key1,
        Key2,
        Key3,
        Key4,
        Key5,
        Key6,
        Key7,
        Key8,
        Key9,
        Key0,
        Escape,
        F1,
        F2,
        F3,
        F4,
        F5,
        F6,
        F7,
        F8,
        F9,
        F10,
        F11,
        F12,
        F13,
        F14,
        F15,
        F16,
        F17,
        F18,
        F19,
        F20,
        F21,
        F22,
        F23,
        F24,
        Insert,
        Home,
        Delete,
        End,
        PageDown,
        PageUp,
        Left,
        Up,
        Right,
        Down,
        Back,
        Return,
        Space,
        Numlock,
        Numpad0,
        Numpad1,
        Numpad2,
        Numpad3,
        Numpad4,
        Numpad5,
        Numpad6,
        Numpad7,
        Numpad8,
        Numpad9,
        NumpadAdd,
        NumpadDivide,
        NumpadDecimal,
        NumpadComma,
        NumpadEnter,
        NumpadEquals,
        NumpadMultiply,
        NumpadSubtract,
        AbntC1,
        AbntC2,
        Apostrophe,
        Apps,
        Asterisk,
        At,
        Ax,
        Backslash,
        Calculator,
        Capital,
        Colon,
        Comma,
        Convert,
        Equals,
        Grave,
        Kana,
        Kanji,
        LAlt,
        LBracket,
        LControl,
        LShift,
        LWin,
        Mail,
        MediaSelect,
        MediaStop,
        Minus,
        Mute,
        MyComputer,
        NavigateForward,
        NavigateBackward,
        NextTrack,
        NoConvert,
        OEM102,
        Period,
        PlayPause,
        Plus,
        Power,
        PrevTrack,
        RAlt,
        RBracket,
        RControl,
        RShift,
        RWin,
        Semicolon,
        Slash,
        Sleep,
        Stop,
        Sysrq,
        Tab,
        Underline,
        Unlabeled,
        VolumeDown,
        VolumeUp,
        Wake,
        WebBack,
        WebFavorites,
        WebForward,
        WebHome,
        WebRefresh,
        WebSearch,
        WebStop,
        Yen,
        Copy,
        Paste,
        Cut
    ];
}
