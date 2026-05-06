use secrecy::SecretString;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationPollResult {
    Pending,
    Finished(Result<(), String>),
}

pub trait UnlockPreparation {
    type PreparedSession;

    fn prepare_unlock(&self, password: &SecretString) -> Result<Self::PreparedSession, String>;
}

pub trait UnlockCompletion<PreparedSession> {
    fn finish_unlock(&mut self, password: SecretString, prepared: PreparedSession);
}

pub trait VerificationHandle {
    fn poll_verification(&self) -> VerificationPollResult;
}

pub trait UserPresenceVerifier {
    type PendingVerification: VerificationHandle;

    fn begin_verification(&self) -> Result<Self::PendingVerification, String>;
}
