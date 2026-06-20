use std::{
    io::BufRead,
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub(super) struct ProcessIdentity {
    pub(super) process_id: u32,
    pub(super) instance_id: Uuid,
    pub(super) started_at_epoch_ms: u64,
}

impl ProcessIdentity {
    pub(super) fn new() -> Self {
        Self {
            process_id: std::process::id(),
            instance_id: Uuid::new_v4(),
            started_at_epoch_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum InputEvent {
    Line(String),
    Eof,
    ReadError(String),
}

pub(super) fn spawn_input_reader<R>(mut reader: R) -> Receiver<InputEvent>
where
    R: BufRead + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = sender.send(InputEvent::Eof);
                    break;
                }
                Ok(_) => {
                    if sender.send(InputEvent::Line(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(InputEvent::ReadError(error.to_string()));
                    break;
                }
            }
        }
    });
    receiver
}

pub(super) fn next_input_event(
    receiver: &Receiver<InputEvent>,
    pump_interval: Duration,
    mut pump_messages: impl FnMut(),
) -> Result<InputEvent, String> {
    loop {
        match receiver.recv_timeout(pump_interval) {
            Ok(event) => return Ok(event),
            Err(RecvTimeoutError::Timeout) => pump_messages(),
            Err(RecvTimeoutError::Disconnected) => {
                return Err("El lector de stdin se desconectó inesperadamente.".to_owned());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, BufReader, Cursor, Read},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };

    use super::{InputEvent, ProcessIdentity, next_input_event, spawn_input_reader};

    #[test]
    fn process_identity_is_stable_and_non_empty() {
        let identity = ProcessIdentity::new();

        assert_eq!(identity.process_id, std::process::id());
        assert!(!identity.instance_id.is_nil());
        assert!(identity.started_at_epoch_ms > 0);
    }

    #[test]
    fn input_reader_emits_lines_and_eof() {
        let receiver = spawn_input_reader(BufReader::new(Cursor::new(b"one\ntwo\n".to_vec())));

        assert_eq!(
            receiver.recv().expect("first event"),
            InputEvent::Line("one\n".to_owned())
        );
        assert_eq!(
            receiver.recv().expect("second event"),
            InputEvent::Line("two\n".to_owned())
        );
        assert_eq!(receiver.recv().expect("eof event"), InputEvent::Eof);
    }

    #[test]
    fn waiting_for_input_keeps_pumping_owner_thread() {
        let (sender, receiver) = mpsc::channel();
        let pump_count = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&pump_count);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(60));
            sender
                .send(InputEvent::Line("ping\n".to_owned()))
                .expect("receiver should remain connected");
        });

        let event = next_input_event(&receiver, Duration::from_millis(5), || {
            observed.fetch_add(1, Ordering::SeqCst);
        })
        .expect("input event");

        assert_eq!(event, InputEvent::Line("ping\n".to_owned()));
        assert!(pump_count.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn disconnected_input_channel_is_reported() {
        let (sender, receiver) = mpsc::channel::<InputEvent>();
        drop(sender);

        let error = next_input_event(&receiver, Duration::from_millis(1), || {})
            .expect_err("disconnect should fail");

        assert!(error.contains("desconectó"));
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("synthetic read failure"))
        }
    }

    #[test]
    fn input_reader_emits_read_errors() {
        let receiver = spawn_input_reader(BufReader::new(FailingReader));

        let event = receiver.recv().expect("read error event");
        assert!(matches!(
            event,
            InputEvent::ReadError(message) if message.contains("synthetic read failure")
        ));
    }
}
