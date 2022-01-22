use internment::Intern;

// #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
// pub struct Step(usize)

// TODO path as linear combination

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathRange {
    path_name: Intern<String>,

    start_step: usize,
    end_step: usize,

    reverse: bool,
}

impl PathRange {
    pub fn new(path: &str, start_step: usize, end_step: usize) -> Self {
        let reverse = start_step > end_step;

        Self {
            path_name: Intern::from(path),

            start_step: start_step.min(end_step),
            end_step: start_step.max(end_step),

            reverse,
        }
    }

    pub fn is_reverse(&self) -> bool {
        self.reverse
    }

    pub fn first(&self) -> usize {
        if self.reverse {
            self.end_step
        } else {
            self.start_step
        }
    }

    pub fn last(&self) -> usize {
        if self.reverse {
            self.start_step
        } else {
            self.end_step
        }
    }

    pub fn min(&self) -> usize {
        self.start_step
    }

    pub fn max(&self) -> usize {
        self.end_step
    }

    pub fn is_empty(&self) -> bool {
        self.start_step == self.end_step
    }

    pub fn len(&self) -> usize {
        self.end_step - self.start_step
    }

    pub fn path_name(&self) -> &'static str {
        self.path_name.as_ref()
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        self.start_step..self.end_step
    }
}

/*
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hashh)]
pub struct PathPosRange {
    path: Intern<String>,

    start_pos: usize,
    end_pos: usize,
}
*/
