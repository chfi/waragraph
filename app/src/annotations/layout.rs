use std::collections::BTreeMap;
use std::sync::Arc;

use waragraph_core::graph::{Bp, PathId};

use super::AnnotationCache;

pub struct AnnotationLayout1D<T> {
    paths: Vec<PathId>,

    cache: Arc<AnnotationCache<T>>,

    record_aabbs: BTreeMap<usize, egui::Rect>,
}
