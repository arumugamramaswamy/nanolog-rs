mod ring_buf;
mod shareable;

pub use ring_buf::Panic;
pub use ring_buf::Spin;
pub use ring_buf::WaitStrategy;

pub const RINGBUF_SIZE: usize = 1024 * 1024;
#[derive(Clone)]
pub struct Logger<W: 'static>(
    shareable::ShareableWriter<ring_buf::RingBuf<RINGBUF_SIZE>>,
    std::marker::PhantomData<W>,
);

impl<W: ring_buf::WaitStrategy> Logger<W> {
    pub fn new(w: shareable::ShareableWriter<ring_buf::RingBuf<RINGBUF_SIZE>>) -> Self {
        Self(w, std::marker::PhantomData)
    }
    pub fn write(&mut self, buf: &[u8]) {
        self.0.write(|rb| rb.write::<W>(buf));
    }

    pub fn commit_write(&mut self) {
        self.0.write(|rb| rb.commit_write())
    }
}

pub struct LogReader(shareable::ShareableReader<ring_buf::RingBuf<RINGBUF_SIZE>>);
impl LogReader {
    pub fn read(&self, buf: &mut [u8]) -> usize {
        // TODO: see if this causes any problems
        let mut a = 0;
        self.0.read(|rb| {
            a = rb.read_all(buf);
        });
        a
    }
}

pub fn create_log_pair<W: ring_buf::WaitStrategy>() -> (Logger<W>, LogReader) {
    // let (r, w) =
    //     shareable::new_shareable_reader_and_writer(ring_buf::RingBuf::<RINGBUF_SIZE>::new());
    // (Logger::<W>::new(w), LogReader(r))

    let (r, w) =
        shareable::new_shareable_reader_and_writer_from_boxed_unsafe(ring_buf::RingBuf::<
            RINGBUF_SIZE,
        >::boxed_unsafe_cell_new(
        ));
    (Logger::<W>::new(w), LogReader(r))
}
