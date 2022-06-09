use rhai::plugin::*;

#[export_module]
pub mod rhai_module {
    pub type PangenomeView = crate::geometry::view::PangenomeView;

    #[rhai_fn(get = "len", pure)]
    pub fn get_len(view: &mut PangenomeView) -> i64 {
        view.len().0 as i64
    }
    #[rhai_fn(get = "offset", pure)]
    pub fn get_offset(view: &mut PangenomeView) -> i64 {
        view.offset().0 as i64
    }

    #[rhai_fn(get = "max", pure)]
    pub fn get_max(view: &mut PangenomeView) -> i64 {
        view.max().0 as i64
    }

    #[rhai_fn(set = "offset")]
    pub fn set_offset(view: &mut PangenomeView, offset: i64) {
        *view = view.set_offset(view.max().0 - view.len().0);
    }
}
