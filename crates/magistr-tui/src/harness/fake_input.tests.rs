use super::*;

const MS: fn(u64) -> Duration = Duration::from_millis;

fn input(script: Script) -> FakeInput {
    FakeInput::new(script, Arc::new(Mutex::new(Size::new(80, 24))))
}

#[test]
fn bytes_due_now_are_read_at_once_and_a_silent_script_times_out() {
    let mut input = input(Script::new().text(MS(0), "hello"));
    let mut buf = [0u8; 16];
    assert_eq!(input.read(&mut buf, MS(100)).expect("read"), Some(5));
    assert_eq!(&buf[..5], b"hello");
    let began = Instant::now();
    assert_eq!(input.read(&mut buf, MS(20)).expect("read"), None);
    assert!(began.elapsed() >= MS(20));
    assert_eq!(input.remaining(), 0);
}

#[test]
fn a_small_buffer_gets_the_rest_on_the_next_read() {
    let mut input = input(Script::new().text(MS(0), "abcdef"));
    let mut buf = [0u8; 4];
    assert_eq!(input.read(&mut buf, MS(10)).expect("read"), Some(4));
    assert_eq!(&buf, b"abcd");
    assert_eq!(input.read(&mut buf, MS(10)).expect("read"), Some(2));
    assert_eq!(&buf[..2], b"ef");
}

#[test]
fn a_future_step_waits_until_due_within_the_timeout() {
    let mut input = input(Script::new().text(MS(30), "x"));
    let mut buf = [0u8; 4];
    let began = Instant::now();
    assert_eq!(input.read(&mut buf, MS(10)).expect("read"), None);
    assert!(began.elapsed() < MS(30));
    assert_eq!(input.read(&mut buf, MS(500)).expect("read"), Some(1));
    assert!(began.elapsed() >= MS(30));
}

#[test]
fn a_resize_reads_as_a_timeout_and_changes_the_size() {
    let size = Arc::new(Mutex::new(Size::new(80, 24)));
    let mut input = FakeInput::new(
        Script::new()
            .resize(MS(0), Size::new(100, 30))
            .text(MS(0), "k"),
        Arc::clone(&size),
    );
    let mut buf = [0u8; 4];
    assert_eq!(input.read(&mut buf, MS(10)).expect("read"), None);
    assert_eq!(input.size().expect("size"), Size::new(100, 30));
    assert_eq!(*size.lock().expect("lock"), Size::new(100, 30));
    assert_eq!(input.read(&mut buf, MS(10)).expect("read"), Some(1));
}

#[test]
fn closing_answers_end_of_file_for_ever() {
    let mut input = input(Script::new().close(MS(0)).text(MS(0), "late"));
    let mut buf = [0u8; 4];
    assert_eq!(input.read(&mut buf, MS(10)).expect("read"), Some(0));
    assert_eq!(input.read(&mut buf, MS(10)).expect("read"), Some(0));
}
