use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

#[derive(Default)]
pub struct ContextState {
    // frame_ctx: Vec<ContextVal<Box<dyn ContextValue>>>, //
    open_frame:
        HashMap<std::any::TypeId, Vec<ContextVal<Box<dyn std::any::Any>>>>,

    ready_frame:
        HashMap<std::any::TypeId, Vec<ContextVal<Box<dyn std::any::Any>>>>,

    type_names: HashMap<std::any::TypeId, Arc<String>>,
}

impl ContextState {
    pub fn start_frame(&mut self) {
        self.ready_frame = std::mem::take(&mut self.open_frame);
    }

    pub fn register_type_name<T: std::any::Any>(&mut self, name: &str) {
        let tid = std::any::TypeId::of::<T>();
        self.type_names.insert(tid, Arc::new(name.to_string()));
    }

    pub fn set<'tags, T: std::any::Any>(
        &mut self,
        source: &str,
        tags: impl IntoIterator<Item = &'tags str>,
        value: T,
    ) {
        let tags = Tags {
            set: tags.into_iter().map(String::from).collect(),
        };

        let ctx_meta = ContextMeta {
            source: source.to_string(),
            tags,
        };

        let data = Box::new(value);

        let ctx_val = ContextVal {
            data: data as _,
            meta: ctx_meta,
        };

        let tid = std::any::TypeId::of::<T>();
        self.open_frame.entry(tid).or_default().push(ctx_val);
    }

    pub fn get<'a, 'b, T: std::any::Any>(
        &'a self,
        query: ContextQuery<'b, T>,
    ) -> Option<&'a dyn ContextValue> {
        let tid = std::any::TypeId::of::<T>();
        let values = self.ready_frame.get(&tid)?;

        values
            .iter()
            .filter(|&v| query.matches(v))
            .map(|v| v as &'a dyn ContextValue)
            .next()
    }
}

pub struct ContextQuery<'a, T: std::any::Any> {
    source: Option<&'a str>,
    tags: BTreeSet<&'a str>,

    _data: std::marker::PhantomData<T>,
}

impl<'a, T: std::any::Any> ContextQuery<'a, T> {
    pub fn from_source(source: &'a str) -> Self {
        ContextQuery {
            source: Some(source),
            tags: BTreeSet::default(),
            _data: std::marker::PhantomData,
        }
    }

    pub fn from_source_tags(
        source: &'a str,
        tags: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        ContextQuery {
            source: Some(source),
            tags: tags.into_iter().collect(),
            _data: std::marker::PhantomData,
        }
    }

    fn matches<V: ContextValue>(&self, value: &V) -> bool {
        if value.type_id() != std::any::TypeId::of::<T>() {
            return false;
        }

        let meta = value.meta();

        if let Some(src) = self.source {
            if src != &meta.source {
                return false;
            }
        }

        let all_tags_present =
            self.tags.iter().all(|&t| meta.tags.set.contains(t));

        if !all_tags_present {
            return false;
        }

        true
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tags {
    pub set: BTreeSet<String>,
}

pub struct ContextMeta {
    source: String,
    tags: Tags,
}

pub struct ContextVal<T> {
    data: T,
    meta: ContextMeta,
}

pub trait ContextValue {
    fn data(&self) -> &dyn std::any::Any;
    fn data_mut(&mut self) -> &mut dyn std::any::Any;
    fn type_id(&self) -> std::any::TypeId;

    fn meta(&self) -> &ContextMeta;

    // fn source(&self) -> ()
    // fn meta
}

pub trait ContextValueExtra: ContextValue {
    fn data_as<T: std::any::Any>(&self) -> Option<&T> {
        self.data().downcast_ref::<T>()
    }

    fn data_mut_as<T: std::any::Any>(&mut self) -> Option<&mut T> {
        self.data_mut().downcast_mut::<T>()
    }
}

impl<T: ContextValue> ContextValueExtra for T {}

impl<T: std::any::Any> ContextValue for ContextVal<T> {
    fn data(&self) -> &dyn std::any::Any {
        &self.data as _
    }

    fn data_mut(&mut self) -> &mut dyn std::any::Any {
        &mut self.data as _
    }

    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<T>()
    }

    fn meta(&self) -> &ContextMeta {
        &self.meta
    }
}
