use nanolog_rs_common::nanolog_logger::LogReader;
use nanolog_rs_proc_macro::{nanolog, setup_nanolog};
// TODO: temp for docs
pub use std::alloc;
pub use std::boxed;
use std::cell;
pub use std::sync;
use std::sync::atomic::AtomicBool;
use std::thread;
pub use std::time;
use std::time::Duration;

pub static LOGGER_SENDER: sync::Mutex<
    cell::OnceCell<
        sync::mpsc::Sender<
            ::nanolog_rs_common::nanolog_logger::RingBufferLogReader<
                ::nanolog_rs_common::nanolog_logger::Spin,
            >,
        >,
    >,
> = sync::Mutex::new(cell::OnceCell::new());

static ALL_THREADS_SETUP: AtomicBool = AtomicBool::new(false);

setup_nanolog!();

fn main() {
    // startup code
    let start_instant = std::time::Instant::now();
    let (logger_sender, logger_receiver) = std::sync::mpsc::channel();
    {
        let sender = crate::LOGGER_SENDER.lock().unwrap();
        sender.set(logger_sender).unwrap();
    }

    let t1 = std::thread::Builder::new()
        .name("t1".to_string())
        .spawn(|| {
            println!("Creating thread 1");
            ::affinity::set_thread_affinity([13]).unwrap();
            let mut logger = setup_logger();

            let a = 1.1;
            let b = 1;

            for x in 0.. {
                nanolog!(&mut logger, "[T1] Hello, world! %f %d", a, x);
                // std::thread::sleep(std::time::Duration::from_nanos(1))
            }
        })
        .unwrap();
    let t2 = std::thread::Builder::new()
        .name("t2".to_string())
        .spawn(|| {
            println!("Creating thread 2");
            ::affinity::set_thread_affinity([14]).unwrap();
            let mut logger = setup_logger();

            let a = 1.1;
            let b = 1;

            // TODO:
            // Further improvements:
            //   - coalesce writes to stdout / disk
            //   - api ergonomics (maybe wrapper around thread_spawn that sets up the logger)
            //   - tokio::main equivalent

            for x in 0.. {
                // nanolog!(&mut logger, "[T2] Hello, world!");
                nanolog!(&mut logger, "[T2] Hello, world! %f %d", a, x);
                // std::thread::sleep(std::time::Duration::from_nanos(1))
            }
        })
        .unwrap();
    ::affinity::set_thread_affinity([15]).unwrap();

    let mut readers = vec![];
    let mut log_reader_buf = [0; ::nanolog_rs_common::nanolog_logger::RINGBUF_SIZE];

    const NUM_THREADS: usize = 2;
    // 2 threads
    for _ in 0..NUM_THREADS {
        readers.push(logger_receiver.recv().unwrap());
        println!("new!");
    }
    // setup complete
    ALL_THREADS_SETUP.store(true, sync::atomic::Ordering::Release);

    let start = std::time::Instant::now();
    while std::time::Instant::now().duration_since(start) < Duration::from_secs(10) {
        // loop {
        for r in readers.iter() {
            let n = r.read(&mut log_reader_buf);
            // println!("{n}");
            if n > 32 {
                // TODO: this is v temporary
                nanolog_internal::decode_buf(&start_instant, &log_reader_buf[n - 32..n]);
            }
        }
        if t1.is_finished() || t2.is_finished() {
            println!("problem!");
            break;
        }
        // thread::yield_now();
        thread::sleep(Duration::from_nanos(50));
    }
}

fn setup_logger(
) -> ::nanolog_rs_common::nanolog_logger::RingBufferLogger<::nanolog_rs_common::nanolog_logger::Spin>
{
    let (logger, log_reader) = ::nanolog_rs_common::nanolog_logger::create_log_pair();
    {
        let sender = crate::LOGGER_SENDER.lock().unwrap();
        sender
            .get()
            .expect("log reader channel must be set up at init time")
            .send(log_reader)
            .unwrap();
    }
    while !ALL_THREADS_SETUP.load(sync::atomic::Ordering::Acquire) {}
    logger
}
