use nanolog_rs_common::nanolog_logger::LogReader;
use nanolog_rs_proc_macro::nanolog;
use std::time::Duration;

macro_rules! setup_nanolog {
    ($rb_size:expr,$type:path) => {
        mod nanolog_internal {
            include!(concat!(env!("OUT_DIR"), "/source_files.rs"));

            pub type Logger =
                ::nanolog_rs_common::nanolog_logger::SharedRingBufferWriter<$rb_size, $type>;
            pub type LogReader =
                ::nanolog_rs_common::nanolog_logger::SharedRingBufferReader<$rb_size>;

            pub static LOGGER_SENDER: ::std::sync::OnceLock<::std::sync::mpsc::Sender<LogReader>> =
                ::std::sync::OnceLock::new();

            pub fn setup_logger() -> Logger {
                let (log_reader, logger) =
                    ::nanolog_rs_common::nanolog_logger::create_reader_writer_pair();
                {
                    LOGGER_SENDER
                        .get()
                        .expect("log reader channel must be set up at init time")
                        .send(log_reader)
                        .unwrap();
                }
                logger
            }
        }
    };
}
const RINGBUF_SIZE: usize = 1024 * 1024;

setup_nanolog!(
    { crate::RINGBUF_SIZE },
    ::nanolog_rs_common::nanolog_logger::Spin
);

pub fn create_thread(
    name: &'static str,
    affinity: Vec<usize>,
    f: impl FnOnce(nanolog_internal::Logger) + Send + 'static,
) -> ::std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(|| {
            let logger = nanolog_internal::setup_logger();
            ::affinity::set_thread_affinity(affinity).unwrap();
            f(logger)
        })
        .unwrap()
}

fn main() {
    // startup code
    let start_instant = std::time::Instant::now();
    let (logger_sender, logger_receiver) = std::sync::mpsc::channel();
    nanolog_internal::LOGGER_SENDER.set(logger_sender).unwrap();

    let t1 = create_thread("T1", vec![13], |mut logger| {
        let a = 1.1;
        let b = 1;

        for x in 0..100_000_000 {
            nanolog!(&mut logger, "[T1] Hello, world!");
            // nanolog!(&mut logger, "[T1] Hello, world! %f %d", a, x);
            // std::thread::sleep(std::time::Duration::from_nanos(1))
        }
    });
    let t2 = create_thread("T2", vec![14], |mut logger| {
        let a = 1.1;
        let b = 1;

        // TODO:
        // Further improvements:
        //   - generate metadata file on setup <--- setup macro seems most important
        //   - coalesce writes to stdout / disk
        //   - api ergonomics (maybe wrapper around thread_spawn that sets up the logger)
        //   - tokio::main equivalent

        for x in 0..100_000_000 {
            nanolog!(&mut logger, "[T2] Hello, world!");
            // nanolog!(&mut logger, "[T2] Hello, world! %f %d", a, x);
            // std::thread::sleep(std::time::Duration::from_nanos(1))
        }
    });
    ::affinity::set_thread_affinity([15]).unwrap();

    let mut readers = vec![];
    let mut log_reader_buf = [0; RINGBUF_SIZE];

    const NUM_THREADS: usize = 2;
    // 2 threads
    for _ in 0..NUM_THREADS {
        readers.push(logger_receiver.recv().unwrap());
        println!("new!");
    }

    let mut out =
        std::io::BufWriter::with_capacity(4 * RINGBUF_SIZE, std::fs::File::create("logs").unwrap());
    let start = std::time::Instant::now();
    loop {
        for r in readers.iter_mut() {
            let n = r.read(&mut log_reader_buf);
            if n > 0 {
                nanolog_internal::decode_buf(&mut out, &start_instant, &log_reader_buf[..n]);
            }
        }
        if t1.is_finished() || t2.is_finished() {
            println!("problem!");
            break;
        }
    }
    println!("{:?}", std::time::Instant::now().duration_since(start));
}
