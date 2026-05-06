use std::{
    any::Any,
    panic::{self, AssertUnwindSafe},
    sync::mpsc,
};

use secrecy::SecretString;

use crate::ports::{UserPresenceVerifier, VerificationHandle, VerificationPollResult};

pub struct PendingPrepareUnlock<PreparedSession> {
    receiver: mpsc::Receiver<Result<PreparedUnlock<PreparedSession>, String>>,
}

pub struct PreparedUnlock<PreparedSession> {
    pub password: SecretString,
    pub session: PreparedSession,
}

pub enum PrepareUnlockPoll<PreparedSession> {
    Pending,
    Finished(Result<PreparedUnlock<PreparedSession>, String>),
}

pub struct PendingUnlockFlow<PreparedSession, PendingVerification>
where
    PendingVerification: VerificationHandle,
{
    password: SecretString,
    session: PreparedSession,
    verification: PendingVerification,
}

impl<PreparedSession> PendingPrepareUnlock<PreparedSession> {
    pub fn poll(&self) -> PrepareUnlockPoll<PreparedSession> {
        match self.receiver.try_recv() {
            Ok(result) => PrepareUnlockPoll::Finished(result),
            Err(mpsc::TryRecvError::Empty) => PrepareUnlockPoll::Pending,
            Err(mpsc::TryRecvError::Disconnected) => PrepareUnlockPoll::Finished(Err(
                "La preparación del desbloqueo terminó de forma inesperada.".to_owned(),
            )),
        }
    }
}

impl<PreparedSession, PendingVerification> PendingUnlockFlow<PreparedSession, PendingVerification>
where
    PendingVerification: VerificationHandle,
{
    pub fn poll(&self) -> VerificationPollResult {
        self.verification.poll_verification()
    }

    pub fn into_parts(self) -> (SecretString, PreparedSession) {
        (self.password, self.session)
    }
}

pub fn spawn_prepare_unlock<PreparedSession, F>(
    password: SecretString,
    prepare: F,
    context: &'static str,
) -> PendingPrepareUnlock<PreparedSession>
where
    PreparedSession: Send + 'static,
    F: FnOnce(&SecretString) -> Result<PreparedSession, String> + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            prepare(&password).map(|session| PreparedUnlock { password, session })
        }))
        .unwrap_or_else(|payload| {
            Err(format!(
                "{context} terminó por panic: {}",
                panic_message(payload)
            ))
        });

        let _ = sender.send(result);
    });

    PendingPrepareUnlock { receiver }
}

pub fn begin_unlock_verification<PreparedSession, Verifier>(
    prepared: PreparedUnlock<PreparedSession>,
    verifier: &Verifier,
) -> Result<PendingUnlockFlow<PreparedSession, Verifier::PendingVerification>, String>
where
    Verifier: UserPresenceVerifier,
{
    Ok(PendingUnlockFlow {
        password: prepared.password,
        session: prepared.session,
        verification: verifier.begin_verification()?,
    })
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_owned(),
            Err(_) => "panic without string payload".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{UserPresenceVerifier, VerificationHandle, VerificationPollResult};

    #[derive(Clone, Copy)]
    struct ApprovingVerifier;

    struct ReadyVerification;

    impl VerificationHandle for ReadyVerification {
        fn poll_verification(&self) -> VerificationPollResult {
            VerificationPollResult::Finished(Ok(()))
        }
    }

    impl UserPresenceVerifier for ApprovingVerifier {
        type PendingVerification = ReadyVerification;

        fn begin_verification(&self) -> Result<Self::PendingVerification, String> {
            Ok(ReadyVerification)
        }
    }

    #[test]
    fn spawn_prepare_unlock_returns_prepared_session() {
        let pending = spawn_prepare_unlock(
            SecretString::from("secret".to_owned()),
            |_password| Ok::<_, String>("prepared".to_owned()),
            "prepare_unlock",
        );

        loop {
            match pending.poll() {
                PrepareUnlockPoll::Pending => {
                    std::thread::sleep(std::time::Duration::from_millis(5))
                }
                PrepareUnlockPoll::Finished(Ok(prepared)) => {
                    assert_eq!(prepared.session, "prepared");
                    break;
                }
                PrepareUnlockPoll::Finished(Err(error)) => panic!("unexpected error: {error}"),
            }
        }
    }

    #[test]
    fn begin_unlock_verification_keeps_password_and_session() {
        let pending = begin_unlock_verification(
            PreparedUnlock {
                password: SecretString::from("secret".to_owned()),
                session: "prepared".to_owned(),
            },
            &ApprovingVerifier,
        )
        .expect("verification should start");

        assert_eq!(pending.poll(), VerificationPollResult::Finished(Ok(())));
        let (_password, session) = pending.into_parts();
        assert_eq!(session, "prepared");
    }
}
