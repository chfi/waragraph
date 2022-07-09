//! Abstractions for working with annotations

use std::sync::Arc;

use bimap::BiHashMap;
use rustc_hash::FxHashMap;

use crate::{
    console::data::AnnotationSet,
    geometry::ScreenSize,
    graph::{Node, Path, Waragraph},
};

// an annotation layout should be parametric over...
//
//  - the domain space
//       (Bp in 1D cases, probably path ranges in graphs, or even subgraphs)
//  - the image space (probably Euclidean 1D or 2D)
//
//  -
//
//
//  -
//  -
//  -
//  -
pub struct AnnotationLayout {
    //
    //
}

// one instance of this will exist per path that has any annotations; and

// it will need...

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
struct AnnotationSetId(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
struct AnnotationRecordId(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
struct UniqueAnnotationId(usize);

/*

the domain is already fixed: ranges (Bp intervals) on a single path

the codomain will be pixels, in 1D for now

*/
/*
pub struct PathAnnotationLayout {
    path: Path,

    annot_boxes: Vec<ScreenSize>,

    // if multiple rows in a BED file have the same value in the
    // column used for the annotation in question, they may be
    // combined;
    //
    // elements in this vector map directly to annotations that can be
    // displayed, and elements in `unique_annots` must line up with
    // those in `annot_boxes`
    //
    // each element identifies an annotation set and record, which can
    // be used to extract the data in question
    unique_annots: Vec<(AnnotationSetId, AnnotationRecordId),


    //
    annotation_sets: BiHashMap<rhai::ImmutableString, AnnotationSetId>,
}

    */

pub struct AnnotManager {
    path_annots: FxHashMap<Path, PathAnnotations<usize>>,
}

#[derive(Clone)]
pub struct AnnotDomain {
    ranges: Vec<std::ops::Range<usize>>,
}

impl AnnotDomain {
    pub fn from_node_set(
        graph: &Waragraph,
        node_set: &roaring::RoaringBitmap,
    ) -> Self {
        let mut node_ranges = Vec::new();

        let mut range_start = None;
        let mut prev_node = None;
        // let mut cur_start = None;

        for node in node_set.iter() {
            if range_start.is_none() {
                range_start = Some(node as usize);
            }

            if let Some(prev) = prev_node {
                if prev + 1 != node {
                    if let Some(start) = range_start {
                        node_ranges.push(start..node as usize);
                        range_start = None;
                    }
                }
            }

            prev_node = Some(node);
        }

        if let (Some(start), Some(node)) = (range_start, prev_node) {
            //
            node_ranges.push(start..node as usize);
        }

        let ranges = node_ranges
            .into_iter()
            .map(|ix_range| {
                let start_bp = graph.node_sum_lens[ix_range.start];

                let end_start = graph.node_sum_lens[ix_range.end];
                let end_bp = end_start + graph.node_lens[ix_range.end] as usize;

                start_bp..end_bp
            })
            .collect();

        Self { ranges }
    }
}

// pub struct PathAnnotations<D: Copy + PartialEq> {
#[derive(Default, Clone)]
pub struct PathAnnotations<D: Copy + PartialEq> {
    annot_key: Vec<(AnnotationSetId, AnnotationRecordId)>,
    domains: Vec<AnnotDomain>,

    annot_bbox: Vec<ScreenSize>,

    annot_data: Vec<D>,
}

impl<D: Copy + PartialEq> PathAnnotations<D> {
    pub fn add_annotation(
        &mut self,
        graph: &Waragraph,
        // path: Path,
        node_set: roaring::RoaringBitmap,
        // pg_range: std::ops::Range<usize>,
        data: D,
    ) -> usize {
        // get the steps in the path
        // sort them by index
        // combine adjacent indices

        todo!();
        //
    }
}

impl PathAnnotations<usize> {
    pub fn from_bed(bed: &AnnotationSet, path: Path) -> Self {
        let mut res = Self::default();

        res
    }
}
