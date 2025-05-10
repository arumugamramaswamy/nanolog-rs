use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::sync::atomic;

struct IsPowerOf2<const N: usize>;
impl<const N: usize> IsPowerOf2<N> {
    const OK: () = assert!(N & (N - 1) == 0);
}

struct SharedRingBuf<const N: usize> {
    arr: UnsafeCell<[u8; N]>,
    head: atomic::AtomicUsize,
    tail: atomic::AtomicUsize,
}

impl<const N: usize> SharedRingBuf<N> {
    fn new() -> Self {
        let _ = IsPowerOf2::<N>::OK;
        Self {
            arr: [0; N].into(),
            head: 0.into(),
            tail: 0.into(),
        }
    }

    /// caller must guarantee that slices must not overlap with existing mutable slices
    unsafe fn slice(&self, start: usize, len: usize) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts((self.arr.get().cast_const() as *const u8).add(start), len)
        }
    }

    /// caller must guarantee that mutable slices are exclusive (no other slice overlaps with it)
    unsafe fn slice_mut(&self, start: usize, len: usize) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut((self.arr.get() as *mut u8).add(start), len) }
    }
}
unsafe impl<const N: usize> Sync for SharedRingBuf<N> {}

pub trait LogReader {
    fn read(&mut self, buf: &mut [u8]) -> usize;
}

pub struct SharedRingBufferReader<const N: usize> {
    rb: &'static SharedRingBuf<N>,
    reader_head: usize, // known
}

unsafe impl<const N: usize> Send for SharedRingBufferReader<N> {}

impl<const N: usize> LogReader for SharedRingBufferReader<N> {
    /// - the buffer must be size N (this is because upto N bytes can be returned by read all)
    /// - the function returns the number of bytes read
    fn read(&mut self, buf: &mut [u8]) -> usize {
        let tail = self.rb.tail.load(atomic::Ordering::Acquire); // other thread writes this value
                                                                 // so I need to load it as acquire

        let n = tail - self.reader_head;
        if n == 0 {
            return 0;
        }

        let start = self.reader_head % N;
        let end = tail % N;

        if start < end {
            buf[..n].copy_from_slice(unsafe { self.rb.slice(start, n) });
        } else {
            let n_temp = N - start;
            buf[..n_temp].copy_from_slice(unsafe { self.rb.slice(start, n_temp) });
            buf[n_temp..n].copy_from_slice(unsafe { self.rb.slice(0, end) });
        }

        // consumed all the way upto tail, so we can just store it directly
        self.reader_head = tail;
        self.rb.head.store(tail, atomic::Ordering::Release);
        n
    }
}

pub trait Logger {
    fn write(&mut self, buf: &[u8]);
    fn commit_write(&mut self);
}

pub struct SharedRingBufferWriter<const N: usize, WaitStrategy> {
    rb: &'static SharedRingBuf<N>,
    writer_head: usize, // best conservative guess
    writer_tail: usize, // known
    _wait_strategy: PhantomData<WaitStrategy>,
}

trait WithWaitStrategy {
    fn wait_to_write(&mut self, len: usize);
}

pub struct Spin {}
impl<const N: usize> WithWaitStrategy for SharedRingBufferWriter<N, Spin> {
    fn wait_to_write(&mut self, len: usize) {
        loop {
            let n = self.writer_tail - self.writer_head;
            let remaining = N - n;

            if len <= remaining {
                break;
            }
            self.writer_head = self.rb.head.load(atomic::Ordering::Acquire);
        }
    }
}

pub struct Panic {}
impl<const N: usize> WithWaitStrategy for SharedRingBufferWriter<N, Panic> {
    fn wait_to_write(&mut self, len: usize) {
        let n = self.writer_tail - self.rb.head.load(atomic::Ordering::Acquire);
        let remaining = N - n;

        if len > remaining {
            panic!("too much to write");
        }
    }
}

#[allow(private_bounds)]
impl<const N: usize, W> Logger for SharedRingBufferWriter<N, W>
where
    SharedRingBufferWriter<N, W>: WithWaitStrategy,
{
    /// copy from buf to the ring buffer
    /// - buf must be of length <= N
    /// - if not enough space is available to write, the WaitStrategy determines what happens
    ///     - if WaitStrategy = Panic, the writer panics
    ///     - if WaitStrategy = Spin, the writer spins until more space is available
    fn write(&mut self, buf: &[u8]) {
        if buf.is_empty() {
            return;
        }

        self.wait_to_write(buf.len());

        let wrapped_tail = self.writer_tail % N;

        if wrapped_tail + buf.len() <= N {
            unsafe { self.rb.slice_mut(wrapped_tail, buf.len()) }.copy_from_slice(buf);
        } else {
            let n_temp = N - wrapped_tail;
            unsafe { self.rb.slice_mut(wrapped_tail, n_temp) }.copy_from_slice(&buf[..n_temp]);
            unsafe { self.rb.slice_mut(0, buf.len() - n_temp) }
                .copy_from_slice(&buf[n_temp..buf.len()]);
        }

        self.writer_tail += buf.len();
    }

    /// complete write by updating the tail to the latest index (this lets the reader know that it
    /// can read more data)
    fn commit_write(&mut self) {
        self.rb
            .tail
            .store(self.writer_tail, atomic::Ordering::Release);
    }
}

pub fn create_reader_writer_pair<const N: usize, W>(
) -> (SharedRingBufferReader<N>, SharedRingBufferWriter<N, W>) {
    let rb: &'static _ = Box::leak(Box::new(SharedRingBuf::new()));
    (
        SharedRingBufferReader { rb, reader_head: 0 },
        SharedRingBufferWriter {
            rb,
            writer_tail: 0,
            writer_head: 0,
            _wait_strategy: PhantomData,
        },
    )
}
