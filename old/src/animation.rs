use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub type AnimFn = Arc<dyn Fn(f32) + Send + Sync + 'static>;

pub struct AnimHandler {
    anim_register: HashMap<rhai::ImmutableString, AnimFn>,

    init_instant: Instant,
    prev_instant: Instant,

    frame: usize,
}

impl AnimHandler {
    pub fn initialize() -> Self {
        let now = Instant::now();

        Self {
            anim_register: HashMap::default(),
            init_instant: now,
            prev_instant: now,
            frame: 0,
        }
    }

    pub fn update(&mut self) {
        let dt = self.prev_instant.elapsed().as_secs_f32();
        self.prev_instant = Instant::now();

        for f in self.anim_register.values() {
            f(dt)
        }

        self.frame += 1;
    }

    pub fn register<F>(&mut self, name: &str, f: F) -> Option<()>
    where
        F: Fn(f32) + Send + Sync + 'static,
    {
        if self.anim_register.contains_key(name) {
            return None;
        }

        self.anim_register.insert(name.into(), Arc::new(f));

        None
    }

    pub fn unregister(&mut self, name: &str) -> Option<AnimFn> {
        self.anim_register.remove(name)
    }
}
