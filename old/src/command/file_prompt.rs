use super::*;

pub(super) type FilePathPrompt<I> =
    PromptObjectRaw<PathBuf, I, PathBuf, PathBuf>;

#[derive(Default)]
pub(super) struct FilePathPromptConfig {
    pub(super) predicate: Option<Arc<dyn Fn(&std::path::Path) -> bool>>,
    pub(super) current_dir: PathBuf,
}

impl FilePathPromptConfig {
    pub fn into_prompt(self) -> FilePathPrompt<impl Iterator<Item = PathBuf>> {
        let prompt_input = self.current_dir.clone();

        let mut config = self;

        let cwd = Arc::new(RwLock::new(config.current_dir.clone()));

        let select =
            Box::new(move |path: &PathBuf| -> PromptAction<PathBuf, PathBuf> {
                if path.is_dir() {
                    PromptAction::PromptFor(path.to_owned())
                } else {
                    PromptAction::Return(path.to_owned())
                }
            });

        let cwd_ = cwd.clone();

        let display = Box::new(
            move |path: &PathBuf,
                  _rect: ScreenRect|
                  -> glyph_brush::OwnedText {
                let result_scale = 20.0;

                let cwd = cwd_.read();

                if Some(path.as_ref()) == cwd.parent() {
                    OwnedText::new("..")
                } else if let Some(file_name) =
                    path.file_name().and_then(|name| name.to_str())
                {
                    OwnedText::new(file_name).with_scale(result_scale)
                } else if let Some(path) = path.to_str() {
                    OwnedText::new(&format!("{}", path))
                } else {
                    OwnedText::new(&format!("{:?}", path))
                }
            },
        );

        let update_choices = Box::new(move |path: PathBuf| {
            if path.is_dir() {
                let path = path.canonicalize()?;
                config.current_dir = path;
                *cwd.write() = config.current_dir.clone();
            }

            let results = config.current_results()?;

            let predicate = config.predicate.clone();

            let mut path = path;
            let go_up = path.pop().then(|| path);

            let contents = results.filter_map(move |entry| {
                let path = entry.ok()?.path();

                if let Some(predicate) = &predicate {
                    predicate(&path).then(|| path)
                } else {
                    Some(path)
                }
            });

            let mut contents = contents.collect::<Vec<_>>();
            contents.sort();

            let iter = go_up.into_iter().chain(contents);

            Ok(iter)
        });

        let prompt_object = PromptObjectRaw {
            prompt_input,
            select,
            display,
            update_choices,
        };

        prompt_object
    }

    pub fn current_results(&self) -> Result<std::fs::ReadDir> {
        let dir = std::fs::read_dir(&self.current_dir)?;
        Ok(dir)
    }

    pub fn new(current_dir: Option<PathBuf>) -> Result<Self> {
        let current_dir = if let Some(dir) = current_dir {
            dir
        } else {
            std::env::current_dir()?
        };

        Ok(Self {
            predicate: None,
            current_dir,
        })
    }

    pub fn from_ext_whitelist<'a>(
        current_dir: Option<PathBuf>,
        ext_whitelist: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self> {
        use std::ffi::OsString;

        let current_dir = if let Some(dir) = current_dir {
            dir
        } else {
            std::env::current_dir()?
        };

        let whitelist = {
            let set = ext_whitelist
                .into_iter()
                .map(OsString::from)
                .collect::<HashSet<_>>();

            (!set.is_empty()).then(|| set)
        };

        if let Some(whitelist) = whitelist {
            let predicate = Arc::new(move |path: &std::path::Path| {
                if path.is_dir() {
                    return true;
                }

                if let Some(ext) = path.extension() {
                    whitelist.contains(ext)
                } else {
                    true
                }
            });

            Ok(Self {
                predicate: Some(predicate),
                current_dir,
            })
        } else {
            Ok(Self {
                predicate: None,
                current_dir,
            })
        }
    }
}
