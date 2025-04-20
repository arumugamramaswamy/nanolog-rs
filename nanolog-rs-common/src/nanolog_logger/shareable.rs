use std::cell::UnsafeCell;

struct Shareable<T: 'static>(&'static UnsafeCell<T>);

impl<T> Clone for Shareable<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T> Copy for Shareable<T> {}

pub struct ShareableWriter<T: 'static> {
    shareable: Shareable<T>,
}

impl<T> Clone for ShareableWriter<T> {
    fn clone(&self) -> Self {
        Self {
            shareable: self.shareable.clone(),
        }
    }
}

impl<T> ShareableWriter<T> {
    pub fn write<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        let a = unsafe { &mut *self.shareable.0.get() };
        f(a)
    }
}

pub struct ShareableReader<T: 'static> {
    shareable: Shareable<T>,
}

unsafe impl<T> Send for ShareableReader<T> {}

impl<T> ShareableReader<T> {
    pub fn read<F>(&self, f: F)
    where
        F: FnOnce(&T),
    {
        let a = unsafe { &*self.shareable.0.get() };
        f(a)
    }
}

pub fn new_shareable_reader_and_writer<T>(obj: T) -> (ShareableReader<T>, ShareableWriter<T>) {
    let s = Shareable(Box::<_>::leak(Box::new(UnsafeCell::new(obj))));
    let sw = ShareableWriter { shareable: s };
    let sr = ShareableReader { shareable: s };
    (sr, sw)
}

pub fn new_shareable_reader_and_writer_from_boxed_unsafe<T>(
    obj: Box<UnsafeCell<T>>,
) -> (ShareableReader<T>, ShareableWriter<T>) {
    let s = Shareable(Box::<_>::leak(obj));
    let sw = ShareableWriter { shareable: s };
    let sr = ShareableReader { shareable: s };
    (sr, sw)
}
