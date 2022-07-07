//! Abstractions for working with annotations

use std::sync::Arc;

use bimap::BiHashMap;

use crate::{
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
