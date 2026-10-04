//! Observe real tty bytes, never a model-generated expected screen.
use magistr_cockpit::cockpit::Size;
use magistr_tui::term::RealTerminal;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{Mode, OFlags};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct Pty {
    master: OwnedFd,
    slave: OwnedFd,
    parser: vt100::Parser,
    pub wire: Vec<u8>,
    pending: Arc<Mutex<Vec<u8>>>,
    stop: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    pub captures: Vec<(String, String)>,
    pub input_latency: Vec<Duration>,
    pub resize_latency: Vec<Duration>,
}

impl Pty {
    pub fn new(size: Size) -> Self {
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("open pty");
        grantpt(&master).expect("grant pty");
        unlockpt(&master).expect("unlock pty");
        let slave = rustix::fs::open(
            ptsname(&master, Vec::new()).expect("pty name"),
            OFlags::RDWR | OFlags::NOCTTY,
            Mode::empty(),
        )
        .expect("open slave");
        let pending = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let reader = {
            let fd = rustix::io::dup(&master).expect("reader");
            let pending = Arc::clone(&pending);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let timeout = Timespec {
                    tv_sec: 0,
                    tv_nsec: 10_000_000,
                };
                let mut bytes = [0u8; 65_536];
                while !stop.load(Ordering::Acquire) {
                    let mut fds = [PollFd::new(&fd, PollFlags::IN)];
                    if poll(&mut fds, Some(&timeout)).expect("poll") == 0 {
                        continue;
                    }
                    match rustix::io::read(&fd, &mut bytes) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => pending.lock().expect("wire").extend_from_slice(&bytes[..n]),
                    }
                }
            })
        };
        let mut pty = Self {
            master,
            slave,
            parser: vt100::Parser::new(size.rows, size.cols, 1_000),
            wire: Vec::new(),
            pending,
            stop,
            reader: Some(reader),
            captures: Vec::new(),
            input_latency: Vec::new(),
            resize_latency: Vec::new(),
        };
        pty.resize(size);
        pty
    }

    pub fn terminal(&self) -> RealTerminal {
        RealTerminal::open(
            rustix::io::dup(&self.slave).expect("input"),
            rustix::io::dup(&self.slave).expect("output"),
        )
        .expect("real terminal")
    }

    pub fn type_bytes(&self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let n = rustix::io::write(&self.master, bytes).expect("type");
            assert!(n > 0);
            bytes = &bytes[n..];
        }
    }

    pub fn resize(&mut self, size: Size) {
        tcsetwinsize(
            &self.slave,
            Winsize {
                ws_row: size.rows,
                ws_col: size.cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )
        .expect("resize tty");
        self.parser.screen_mut().set_size(size.rows, size.cols);
    }

    pub fn drain(&mut self) {
        let bytes = std::mem::take(&mut *self.pending.lock().expect("wire"));
        self.parser.process(&bytes);
        self.wire.extend_from_slice(&bytes);
    }

    pub fn screen(&mut self) -> String {
        self.drain();
        self.parser.screen().contents()
    }

    pub async fn shows(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            let screen = self.screen();
            if screen.contains(text) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "never showed {text:?}; screen starts: {:?}",
                screen.lines().take(10).collect::<Vec<_>>()
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    pub async fn key(&mut self, bytes: &[u8], expected: &str) {
        let changes_visible_text = !self.screen().contains(expected);
        let began = Instant::now();
        self.type_bytes(bytes);
        self.shows(expected).await;
        if changes_visible_text {
            self.input_latency.push(began.elapsed());
        }
    }

    pub async fn click(&mut self, label: &str) {
        self.shows(label).await;
        let screen = self.screen();
        let (y, line) = screen
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains(label))
            .expect("visible target");
        let x = line[..line.find(label).expect("label")].chars().count();
        self.type_bytes(
            format!("\x1b[<0;{};{}M\x1b[<0;{};{}m", x + 1, y + 1, x + 1, y + 1).as_bytes(),
        );
        tokio::time::sleep(Duration::from_millis(80)).await;
        self.drain();
    }

    pub fn capture(&mut self, name: &str) {
        let screen = self.screen();
        self.captures.push((name.into(), screen));
    }

    pub async fn wire_shows(&mut self, after: usize, bytes: &[u8]) {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            self.drain();
            if self.wire[after..]
                .windows(bytes.len())
                .any(|part| part == bytes)
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "missing terminal bytes {:?}; first {:?}; screen {:?}",
                String::from_utf8_lossy(bytes),
                String::from_utf8_lossy(&self.wire[..self.wire.len().min(1500)]),
                self.parser
                    .screen()
                    .contents()
                    .chars()
                    .take(300)
                    .collect::<String>()
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    pub fn graphics_capture(&mut self, name: &str, size: Size) {
        self.drain();
        let capture = magistr_tui::harness::Capture::new();
        capture.push(&self.wire);
        let images = capture.pixels_before(Instant::now());
        assert!(!images.is_empty(), "actual Kitty placements");
        let mut canvas =
            tiny_skia::Pixmap::new(u32::from(size.cols) * 10, u32::from(size.rows) * 20)
                .expect("bounded capture");
        for image in images {
            let pixels = tiny_skia::PixmapRef::from_bytes(&image.rgba, image.width, image.height)
                .expect("decoded placement");
            canvas.draw_pixmap(
                i32::from(image.col) * 10,
                i32::from(image.row) * 20,
                pixels,
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
        }
        if let Some(dir) = std::env::var_os("MAGISTR_JOURNEY_EVIDENCE") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).expect("evidence directory");
            canvas
                .save_png(dir.join(format!("{name}.png")))
                .expect("actual Kitty capture");
        }
    }

    pub fn persist(&self, prefix: &str) {
        if let Some(dir) = std::env::var_os("MAGISTR_JOURNEY_EVIDENCE") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).expect("evidence directory");
            for (name, screen) in &self.captures {
                std::fs::write(dir.join(format!("{prefix}-{name}.txt")), screen).expect("capture");
            }
            std::fs::write(dir.join(format!("{prefix}.ansi")), &self.wire).expect("tty wire");
        }
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(reader) = self.reader.take() {
            reader.join().expect("tty reader");
        }
    }
}
