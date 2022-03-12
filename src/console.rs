use std::{collections::HashMap, num::NonZeroU32};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{context::VkContext, BufferIx, GpuResources, VkEngine};
use rustc_hash::FxHashMap;

use sled::IVec;
use thunderdome::{Arena, Index};

use sprs::{CsMatI, CsVecI, TriMatI};
use zerocopy::{AsBytes, FromBytes};

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

use bstr::ByteSlice as BstrByteSlice;

use crate::{util::LabelStorage, viewer::ViewDiscrete1D};

#[derive(Default, Clone)]
pub struct Console {
    pub input: String,
    focus: usize,
}

impl Console {
    /*
    pub fn handle_input(
        &mut self,
        input: &winit::event::KeyboardInput,
    ) -> Result<()> {
        // winit::event::ElementState::
        // input.state
        let pressed =
            matches!(input.state, winit::event::ElementState::Pressed);

        if let Some(vk) = input.virtual_keycode {
            match vk {}
        }
    }
    */

    pub fn handle_input(
        &mut self,
        db: &sled::Db,
        txt: &LabelStorage,
        input: ConsoleInput,
    ) -> Result<()> {
        match input {
            ConsoleInput::AppendChar(c) => {
                self.input.push(c);
                // self.input.insert(self.focus, c);
                // self.focus += 1;
            }
            ConsoleInput::Submit => {
                match eval::<rhai::Dynamic>(db, &self.input) {
                    Ok(r) => {
                        //
                        log::warn!("Console result: {:?}", r);
                    }
                    Err(e) => {
                        //
                        log::error!("Console error: {:?}", e);
                    }
                }
                self.input.clear();
                // self.focus = 0;
            }
            ConsoleInput::Backspace => {
                self.input.pop();
                /*
                if self.focus >= 1 {
                    log::error!("backspacing! len: {}", self.input.len());
                    log::error!("focus: {}", self.focus);

                    log::error!("BEFORE input {}", self.input);
                    self.focus -= 1;
                    self.input.remove(self.focus - 1);
                    log::error!(" AFTER input {}", self.input);
                }
                */
            }
        }

        txt.set_text_for(b"console", &self.input)?;

        Ok(())
    }
}

// enum ConsoleInput<'a> {
//     AppendStr(&'a str),
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConsoleInput {
    AppendChar(char),
    Submit,
    Backspace,
    // Delete,
    // Left,
    // Right,
    // InsertChar(char),
    // Home,
    // End,
    // Endline,
}

impl ConsoleInput {
    pub fn from_key_input(ev: &winit::event::KeyboardInput) -> Option<Self> {
        use winit::event::VirtualKeyCode as VK;
        // ev.
        let pressed = matches!(ev.state, winit::event::ElementState::Pressed);

        match ev.virtual_keycode? {
            VK::Key1 => todo!(),
            VK::Key2 => todo!(),
            VK::Key3 => todo!(),
            VK::Key4 => todo!(),
            VK::Key5 => todo!(),
            VK::Key6 => todo!(),
            VK::Key7 => todo!(),
            VK::Key8 => todo!(),
            VK::Key9 => todo!(),
            VK::Key0 => todo!(),
            VK::A => todo!(),
            VK::B => todo!(),
            VK::C => todo!(),
            VK::D => todo!(),
            VK::E => todo!(),
            VK::F => todo!(),
            VK::G => todo!(),
            VK::H => todo!(),
            VK::I => todo!(),
            VK::J => todo!(),
            VK::K => todo!(),
            VK::L => todo!(),
            VK::M => todo!(),
            VK::N => todo!(),
            VK::O => todo!(),
            VK::P => todo!(),
            VK::Q => todo!(),
            VK::R => todo!(),
            VK::S => todo!(),
            VK::T => todo!(),
            VK::U => todo!(),
            VK::V => todo!(),
            VK::W => todo!(),
            VK::X => todo!(),
            VK::Y => todo!(),
            VK::Z => todo!(),
            VK::Return => todo!(),
            VK::Space => todo!(),
            _ => (),
            // VK::Escape => todo!(),
            /*
            VK::F1 => todo!(),
            VK::F2 => todo!(),
            VK::F3 => todo!(),
            VK::F4 => todo!(),
            VK::F5 => todo!(),
            VK::F6 => todo!(),
            VK::F7 => todo!(),
            VK::F8 => todo!(),
            VK::F9 => todo!(),
            VK::F10 => todo!(),
            VK::F11 => todo!(),
            VK::F12 => todo!(),
            VK::F13 => todo!(),
            VK::F14 => todo!(),
            VK::F15 => todo!(),
            VK::F16 => todo!(),
            VK::F17 => todo!(),
            VK::F18 => todo!(),
            VK::F19 => todo!(),
            VK::F20 => todo!(),
            VK::F21 => todo!(),
            VK::F22 => todo!(),
            VK::F23 => todo!(),
            VK::F24 => todo!(),
            VK::Snapshot => todo!(),
            VK::Scroll => todo!(),
            VK::Pause => todo!(),
            */
            /*
            VK::Insert => todo!(),
            VK::Home => todo!(),
            VK::Delete => todo!(),
            VK::End => todo!(),
            VK::PageDown => todo!(),
            VK::PageUp => todo!(),
            VK::Left => todo!(),
            VK::Up => todo!(),
            VK::Right => todo!(),
            VK::Down => todo!(),
            VK::Back => todo!(),
            */
            /*
            VK::Compose => todo!(),
            VK::Caret => todo!(),
            VK::Numlock => todo!(),
            VK::Numpad0 => todo!(),
            VK::Numpad1 => todo!(),
            VK::Numpad2 => todo!(),
            VK::Numpad3 => todo!(),
            VK::Numpad4 => todo!(),
            VK::Numpad5 => todo!(),
            VK::Numpad6 => todo!(),
            VK::Numpad7 => todo!(),
            VK::Numpad8 => todo!(),
            VK::Numpad9 => todo!(),
            VK::NumpadAdd => todo!(),
            VK::NumpadDivide => todo!(),
            VK::NumpadDecimal => todo!(),
            VK::NumpadComma => todo!(),
            VK::NumpadEnter => todo!(),
            VK::NumpadEquals => todo!(),
            VK::NumpadMultiply => todo!(),
            VK::NumpadSubtract => todo!(),
            VK::AbntC1 => todo!(),
            VK::AbntC2 => todo!(),
            */
            /*
            VK::Apostrophe => todo!(),
            VK::Apps => todo!(),
            VK::Asterisk => todo!(),
            VK::At => todo!(),
            VK::Ax => todo!(),
            VK::Backslash => todo!(),
            VK::Calculator => todo!(),
            VK::Capital => todo!(),
            VK::Colon => todo!(),
            VK::Comma => todo!(),
            VK::Convert => todo!(),
            VK::Equals => todo!(),
            VK::Grave => todo!(),
            VK::Kana => todo!(),
            VK::Kanji => todo!(),
            VK::LAlt => todo!(),
            VK::LBracket => todo!(),
            VK::LControl => todo!(),
            VK::LShift => todo!(),
            VK::LWin => todo!(),
            VK::Mail => todo!(),
            VK::MediaSelect => todo!(),
            VK::MediaStop => todo!(),
            VK::Minus => todo!(),
            VK::Mute => todo!(),
            VK::MyComputer => todo!(),
            VK::NavigateForward => todo!(),
            VK::NavigateBackward => todo!(),
            VK::NextTrack => todo!(),
            VK::NoConvert => todo!(),
            VK::OEM102 => todo!(),
            VK::Period => todo!(),
            VK::PlayPause => todo!(),
            VK::Plus => todo!(),
            VK::Power => todo!(),
            VK::PrevTrack => todo!(),
            VK::RAlt => todo!(),
            VK::RBracket => todo!(),
            VK::RControl => todo!(),
            VK::RShift => todo!(),
            VK::RWin => todo!(),
            VK::Semicolon => todo!(),
            VK::Slash => todo!(),
            VK::Sleep => todo!(),
            VK::Stop => todo!(),
            VK::Sysrq => todo!(),
            VK::Tab => todo!(),
            VK::Underline => todo!(),
            VK::Unlabeled => todo!(),
            VK::VolumeDown => todo!(),
            VK::VolumeUp => todo!(),
            VK::Wake => todo!(),
            VK::WebBack => todo!(),
            VK::WebFavorites => todo!(),
            VK::WebForward => todo!(),
            VK::WebHome => todo!(),
            VK::WebRefresh => todo!(),
            VK::WebSearch => todo!(),
            VK::WebStop => todo!(),
            VK::Yen => todo!(),
            VK::Copy => todo!(),
            VK::Paste => todo!(),
            VK::Cut => todo!(),
            */
        }

        None
    }
}

pub fn create_engine(db: &sled::Db) -> rhai::Engine {
    //
    let mut engine = rhai::Engine::new();

    engine.register_type_with_name::<IVec>("IVec");

    engine.register_fn("write_u64", |v: &mut IVec, offset: i64, val: i64| {
        let val = val as u64;
        let o = offset as usize;
        if o + 8 <= v.len() {
            v[o..o + 8].clone_from_slice(&val.to_le_bytes());
        }
    });

    engine.register_fn(
        "write_u64s",
        |v: &mut IVec, offset: i64, vs: rhai::Array| {
            let mut offset = offset as usize;
            for val in vs {
                if let Some(i) = val.try_cast::<i64>() {
                    let i = i as u64;
                    v[offset..offset + 8].clone_from_slice(&i.to_le_bytes());
                    offset += 8;
                }
            }
        },
    );

    engine.register_fn(
        "write_ascii",
        |v: &mut IVec, offset: i64, txt: &str| {
            let offset = offset as usize;
            let bytes = txt.as_bytes();
            v[offset..offset + bytes.len()].clone_from_slice(bytes);
        },
    );

    engine.register_result_fn(
        "subslice",
        |v: &mut IVec, offset: i64, len: i64| {
            let o = offset as usize;
            let l = len as usize;

            if o >= v.len() || o + l > v.len() {
                return Err("offset out of bounds".into());
            }

            Ok(v.subslice(o, l))
        },
    );

    engine.register_result_fn("as_u64", |v: &mut IVec| {
        u64::read_from(v.as_ref()).ok_or("bytestring is not u64".into())
    });

    engine.register_result_fn("as_u32", |v: &mut IVec| {
        u32::read_from(v.as_ref()).ok_or("bytestring is not u32".into())
    });

    let db_ = db.clone();
    engine.register_fn("get", move |k: &str| {
        let k = k.as_bytes();
        let v = db_.get(k).unwrap().unwrap();
        v
    });

    let db_ = db.clone();
    engine.register_fn("set", move |k: &str, v: IVec| {
        // let k = k.as_bytes();
        db_.insert(k, v).unwrap();
        // let v = db_.get(k).unwrap().unwrap();
        // v
    });

    let db_ = db.clone();
    engine.register_fn("view", move || {
        let raw = db_.get(b"view").unwrap().unwrap();
        ViewDiscrete1D::from_bytes(&raw)
    });

    let db_ = db.clone();
    engine.register_fn("set_view_offset", move |new: i64| {
        let offset = new.abs() as usize;
        let raw = db_.get(b"view").unwrap().unwrap();
        let mut view = ViewDiscrete1D::from_bytes(&raw).unwrap();
        view.offset = offset.clamp(0, view.max() - view.len());
        log::warn!("new view offset: {}", view.offset);
        let bytes = view.as_bytes();
        db_.update_and_fetch(b"view", |_| Some(&bytes)).unwrap();
    });

    let db_ = db.clone();
    engine.register_fn("view_offset", move || {
        let raw = db_.get(b"view").unwrap().unwrap();
        let view = ViewDiscrete1D::from_bytes(&raw).unwrap();
        view.offset() as i64
    });
    // let tree =

    engine
}

pub fn eval<T: Clone + Send + Sync + 'static>(
    db: &sled::Db,
    script: &str,
) -> Result<T> {
    let engine = create_engine(db);
    match engine.eval(script) {
        Ok(result) => Ok(result),
        Err(err) => Err(anyhow!("eval err: {:?}", err)),
    }
}
