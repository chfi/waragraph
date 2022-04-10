#![allow(non_upper_case_globals)]

use bstr::ByteSlice;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use parking_lot::RwLock;

use rhai::plugin::*;

use rhai::ImmutableString;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

pub fn create_key_module() -> rhai::Module {
    let mut module: rhai::Module = rhai::exported_module!(key);

    // module.set_var(name, value)

    add_keys(&mut module);

    module
}

#[export_module]
pub mod key {

    use winit::event;

    pub type Key = event::VirtualKeyCode;

    pub const TEST: Key = event::VirtualKeyCode::A;
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
