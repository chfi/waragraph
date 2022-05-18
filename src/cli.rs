use std::path::PathBuf;

use argh::FromArgs;

#[derive(FromArgs)]
/// Arguments for the main waragraph viewer.
pub struct ViewerArgs {
    /// path to GFA file
    #[argh(positional)]
    pub gfa_path: PathBuf,

    /// path to BED file to load, if any
    #[argh(option)]
    pub bed_path: Option<PathBuf>,

    /// column indices into BED file that will be prepared as viz. modes
    #[argh(option)]
    pub bed_columns: Vec<usize>,

    /// script to evaluate on startup
    #[argh(option, long = "run")]
    pub run_script: Option<PathBuf>,
}
