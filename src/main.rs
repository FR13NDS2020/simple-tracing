use std::hash::{Hash, Hasher};
use std::{
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Debug)]
pub struct ProfileResult {
    pub name: String,
    pub start: i64,
    pub end: i64,
    pub thread_id: u32,
}

#[derive(Debug)]
pub struct InstrumentationSession {
    pub name: String,
}

pub struct Instrumentor {
    current_session: Option<InstrumentationSession>,
    output_stream: Option<Mutex<File>>,
    profile_count: usize,
}

lazy_static::lazy_static! {
    static ref INSTRUMENTOR: Arc<Mutex<Instrumentor>> = Arc::new(Mutex::new(Instrumentor::new()));
}

impl Instrumentor {
    fn new() -> Self {
        Instrumentor {
            current_session: None,
            output_stream: None,
            profile_count: 0,
        }
    }

    pub fn begin_session(name: &str, filepath: &str) {
        let mut instrumentor = INSTRUMENTOR.lock().unwrap();
        instrumentor.internal_begin_session(name, filepath);
    }

    pub fn end_session() {
        let mut instrumentor = INSTRUMENTOR.lock().unwrap();
        instrumentor.internal_end_session();
    }

    pub fn write_profile(result: &ProfileResult) {
        let mut instrumentor = INSTRUMENTOR.lock().unwrap();
        instrumentor.internal_write_profile(result);
    }

    fn internal_begin_session(&mut self, name: &str, filepath: &str) {
        if self.current_session.is_some() {
            return;
        }

        if let Ok(file) = File::create(filepath) {
            self.output_stream = Some(Mutex::new(file));
            self.write_header();
            self.current_session = Some(InstrumentationSession {
                name: name.to_string(),
            });
        }
    }

    fn internal_end_session(&mut self) {
        if let Some(ref mut _session) = self.current_session {
            self.write_footer();
            self.output_stream.take().map(|file| {
                drop(file.lock().unwrap());
            });
            self.current_session = None;
            self.profile_count = 0;
        }
    }

    fn internal_write_profile(&mut self, result: &ProfileResult) {
        if let Some(ref mut stream) = self.output_stream {
            let mut stream = stream.lock().unwrap();
            if self.profile_count > 0 {
                write!(stream, ",").unwrap();
            }

            write!(stream, "{{\"cat\":\"function\",\"dur\":{},\"name\":\"{}\",\"ph\":\"X\",\"pid\":0,\"tid\":{},\"ts\":{}}}",
                result.end - result.start,
                result.name.replace('"', "'"),
                result.thread_id,
                result.start,
            ).unwrap();

            stream.flush().unwrap();
            self.profile_count += 1;
        }
    }

    fn write_header(&mut self) {
        if let Some(ref mut stream) = self.output_stream {
            let mut stream = stream.lock().unwrap();
            write!(stream, "{{\"otherData\": {{}},\"traceEvents\":[").unwrap();
            stream.flush().unwrap();
        }
    }

    fn write_footer(&mut self) {
        if let Some(ref mut stream) = self.output_stream {
            let mut stream = stream.lock().unwrap();
            write!(stream, "]}}").unwrap();
            stream.flush().unwrap();
        }
    }
}

impl<'a> Drop for InstrumentationTimer<'a> {
    fn drop(&mut self) {
        if !self.stopped {
            self.stop();
        }
    }
}

pub struct InstrumentationTimer<'a> {
    name: &'a str,
    start_timepoint: Option<Instant>,
    stopped: bool,
}

impl<'a> InstrumentationTimer<'a> {
    pub fn new(name: &'a str) -> Self {
        InstrumentationTimer {
            name,
            start_timepoint: Some(Instant::now()),
            stopped: false,
        }
    }

    pub fn stop(&mut self) {
        if let Some(start_timepoint) = self.start_timepoint.take() {
            let end_timepoint = Instant::now();
            let elapsed = end_timepoint.duration_since(start_timepoint);

            let start = start_timepoint.elapsed().as_micros() as i64;
            let duration = elapsed.as_micros() as i64;

            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::thread::current().id().hash(&mut hasher);
            let thread_id = hasher.finish() as u32;

            Instrumentor::write_profile(&ProfileResult {
                name: self.name.to_string(),
                start,
                end: start + duration,
                thread_id,
            });

            self.stopped = true;
        }
    }
}

#[macro_export]
macro_rules! tracing {
    ($name:expr) => {
        let _timer = InstrumentationTimer::new($name);
    };
}

fn main() {
    // Usage Example:
    // imagine that there is defined your function
    fn do_something() {
        // All you need to do is just add this to your functions that you want to profile
        tracing!("doing something");

        // there is your code
    }
    // Specify session start and end.
    Instrumentor::begin_session("SessionName", "file_name.json");
    // Here is your code.
    do_something();
    Instrumentor::end_session();
    // And voila, you have your profiling data, which you can put in chrome://tracing and clearly
    // see how your aplication is runing
}
