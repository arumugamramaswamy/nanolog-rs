mod ring_buf;
mod shareable;

pub use ring_buf::Panic;
pub use ring_buf::Spin;
use ring_buf::WithWaitStrategy;

pub trait Logger {
    fn write(&mut self, buf: &[u8]);
    fn commit_write(&mut self);
}

pub trait LogReader {
    fn read(&self, buf: &mut [u8]) -> usize;
}

pub const RINGBUF_SIZE: usize = 512 * 1024;
#[derive(Clone)]
pub struct RingBufferLogger<W: 'static>(
    shareable::ShareableWriter<ring_buf::RingBuf<RINGBUF_SIZE, W>>,
);

impl<W> Logger for RingBufferLogger<W>
where
    ring_buf::RingBuf<RINGBUF_SIZE, W>: WithWaitStrategy,
{
    fn write(&mut self, buf: &[u8]) {
        self.0.write(|rb| rb.write(buf));
    }

    fn commit_write(&mut self) {
        self.0.write(|rb| rb.commit_write())
    }
}

pub struct RingBufferLogReader<W: 'static>(
    shareable::ShareableReader<ring_buf::RingBuf<RINGBUF_SIZE, W>>,
);

impl<W> LogReader for RingBufferLogReader<W>
where
    ring_buf::RingBuf<RINGBUF_SIZE, W>: WithWaitStrategy,
{
    fn read(&self, buf: &mut [u8]) -> usize {
        // TODO: see if this causes any problems
        let mut a = 0;
        self.0.read(|rb| {
            a = rb.read_all(buf);
        });
        a
    }
}

pub fn create_log_pair<W>() -> (RingBufferLogger<W>, RingBufferLogReader<W>) {
    let (r, w) =
        shareable::new_shareable_reader_and_writer_from_boxed_unsafe(ring_buf::RingBuf::<
            RINGBUF_SIZE,
            W,
        >::boxed_unsafe_cell_new(
        ));
    (RingBufferLogger(w), RingBufferLogReader(r))
}
