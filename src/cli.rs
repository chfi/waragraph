use std::path::PathBuf;

use argh::FromArgs;

#[derive(FromArgs)]
/// Arguments for the main waragraph viewer.
pub struct ViewerArgs {
    /// path to GFA file
    #[argh(positional)]
    pub gfa_path: PathBuf,

    /// path to BED file to load, if any. results are placed in the
    /// `bed_file` console var
    #[argh(option)]
    pub bed_path: Option<PathBuf>,

    /// column indices into BED file that will be prepared as viz. modes
    #[argh(option, short = 'c')]
    pub bed_column: Vec<usize>,

    /// script to evaluate on startup, results are placed in the
    /// `run_result` console var
    #[argh(option, long = "run")]
    pub run_script: Option<PathBuf>,
}
