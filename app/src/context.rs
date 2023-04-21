use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

pub mod widget;

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

    pub fn debug_print(&self) {
        println!("open_frame: {}", self.open_frame.len());

        println!("ready_frame: {}", self.ready_frame.len());
        for (_tid, ctxs) in self.ready_frame.iter() {
            for ctx in ctxs {
                println!("{:?}", ctx.meta());
            }
        }
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

        let tid = std::any::TypeId::of::<T>();

        let ctx_meta = ContextMeta {
            type_id: tid,
            source: source.to_string(),
            tags,
        };

        let data = Box::new(value);

        let ctx_val = ContextVal {
            data: data as _,
            meta: ctx_meta,
        };

        self.open_frame.entry(tid).or_default().push(ctx_val);
    }

    pub fn query_get_cast<'a, K, T: std::any::Any>(
        &'a self,
        source: Option<K>,
        tags: impl IntoIterator<Item = K>,
    ) -> Option<&'a T>
    where
        K: Ord + AsRef<str>,
    {
        let query = ContextQuery {
            source,
            tags: tags.into_iter().collect(),
            type_id: std::any::TypeId::of::<T>(),
        };

        self.get_cast::<K, T>(&query)
    }

    pub fn get_cast<'a, K, T: std::any::Any>(
        &'a self,
        query: &ContextQuery<K>,
    ) -> Option<&'a T>
    where
        K: Ord + AsRef<str>,
    {
        let values = self.ready_frame.get(&query.type_id)?;

        values.iter().find_map(|v| {
            query
                .matches_str(v)
                .then_some(v)?
                .data()
                .downcast_ref::<T>()
        })
    }

    pub fn get<'a, K>(
        &'a self,
        query: &ContextQuery<K>,
    ) -> Option<&'a dyn ContextValue>
    where
        K: Ord + AsRef<str>,
    {
        let values = self.ready_frame.get(&query.type_id)?;

        values
            .iter()
            .filter(|&v| query.matches_str(v))
            .map(|v| v as &'a dyn ContextValue)
            .next()
    }
}

pub struct ContextQuery<K: Ord> {
    source: Option<K>,
    tags: BTreeSet<K>,

    type_id: std::any::TypeId,
}

impl<K: Ord> ContextQuery<K> {
    pub fn from_source<T: std::any::Any>(source: K) -> Self {
        ContextQuery {
            source: Some(source),
            tags: BTreeSet::default(),
            type_id: std::any::TypeId::of::<T>(),
        }
    }

    pub fn from_source_tags<T: std::any::Any>(
        source: K,
        tags: impl IntoIterator<Item = K>,
    ) -> Self {
        ContextQuery {
            source: Some(source),
            tags: tags.into_iter().collect(),
            type_id: std::any::TypeId::of::<T>(),
        }
    }
    // }

    // impl<'a, T: std::any::Any> ContextQuery<&'a str, T> {
    // impl<K, T: std::any::Any> ContextQuery<K, T> {
    fn matches_str<V>(&self, value: &V) -> bool
    where
        V: ContextValue,
        K: AsRef<str>,
    {
        if value.type_id() != self.type_id {
            return false;
        }

        let meta = value.meta();

        if let Some(src) = self.source.as_ref() {
            if src.as_ref() != &meta.source {
                return false;
            }
        }

        let all_tags_present =
            self.tags.iter().all(|t| meta.tags.set.contains(t.as_ref()));

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

#[derive(Debug)]
pub struct ContextMeta {
    type_id: std::any::TypeId,
    pub source: String,
    pub tags: Tags,
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
// impl<'a, T: ContextValue> ContextValueExtra for &'a dyn T {}

impl ContextValue for ContextVal<Box<dyn std::any::Any>> {
    fn data(&self) -> &dyn std::any::Any {
        self.data.as_ref()
    }

    fn data_mut(&mut self) -> &mut dyn std::any::Any {
        &mut self.data as _
    }

    fn type_id(&self) -> std::any::TypeId {
        self.meta().type_id
    }

    fn meta(&self) -> &ContextMeta {
        &self.meta
    }
}
