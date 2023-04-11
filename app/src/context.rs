pub struct ContextState {
    frame_ctx: Vec<ContextVal<Box<dyn ContextValue>>>, //
}

pub struct ContextMeta {
    source: (),
    meta: (),
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

impl ContextState {
    pub fn start_frame(&mut self) {
        //
        todo!();
    }
}
