use anyhow::Result;
use std::io::prelude::*;
use std::io::BufReader;
use ultraviolet::Vec2;

pub mod viewer_1d;
pub mod viewer_2d;

pub mod spoke;

pub mod app;

pub mod annotations;
pub mod color;
pub mod gpu_cache;
pub mod gui;
pub mod list;

pub mod util;

/*
// commenting since i'm not actually sure if it's useful in this form
pub mod util {
    use ultraviolet as uv;

    pub trait IntoUltraviolet<T> {
        fn into_uv(&self) -> T;
    }

    pub trait IntoEmath<T> {
        fn into_emath(&self) -> T;
    }

    macro_rules! impl_intos {
        ($mint:ty = $emath:ty = $uv:ty) => {
            impl IntoUltraviolet<$uv> for $emath {
                fn into_uv(&self) -> $uv {
                    let tmp = <$mint>::from(*self);
                    <$uv>::from(tmp)
                }
            }

            impl IntoEmath<$emath> for $uv {
                fn into_emath(&self) -> $emath {
                    let tmp = <$mint>::from(*self);
                    <$emath>::from(tmp)
                }
            }
        };
    }

    impl_intos!(mint::Vector2<f32> = egui::Vec2 = uv::Vec2);
}
*/
