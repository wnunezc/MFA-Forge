use mfa_forge_application::{
    ports::VerificationPollResult,
    unlock::{
        PendingPrepareUnlock, PendingUnlockFlow, PrepareUnlockPoll, begin_unlock_verification,
        spawn_prepare_unlock,
    },
};
use secrecy::SecretString;
use zeroize::Zeroize;

use crate::{
    app::ForgeApp,
    i18n::tr,
    platform_auth,
    state::BannerTone,
    vault::{PendingUnlockSession, VaultFacade},
};

pub(crate) type GuiPendingPrepareUnlock = PendingPrepareUnlock<PendingUnlockSession>;
pub(crate) type GuiPendingUnlockFlow =
    PendingUnlockFlow<PendingUnlockSession, platform_auth::PendingVerification>;

impl ForgeApp {
    pub fn unlock_vault(&mut self) {
        if self.pending_prepare.is_some() || self.pending_unlock.is_some() {
            return;
        }

        let mut password = std::mem::take(&mut self.state.loader.password_input);

        if password.trim().is_empty() {
            self.state.loader.error = Some(tr("Enter your master password to unlock."));
            password.zeroize();
            return;
        }

        let password = SecretString::from(std::mem::take(&mut password));
        self.state.loader.error = None;
        self.pending_prepare = Some(spawn_prepare_unlock(
            password,
            |password| match VaultFacade::try_new().map_err(|error| error.to_string()) {
                Ok(vault) => vault.prepare_unlock(password),
                Err(error) => Err(error),
            },
            "Unlock preparation",
        ));
        self.set_banner(
            BannerTone::Info,
            tr("Validating the master password before requesting the additional Windows verification."),
        );
    }

    pub fn is_unlock_pending(&self) -> bool {
        self.pending_prepare.is_some() || self.pending_unlock.is_some()
    }

    pub fn is_unlock_preparing(&self) -> bool {
        self.pending_prepare.is_some()
    }

    pub fn is_unlock_verifying(&self) -> bool {
        self.pending_unlock.is_some()
    }

    pub(crate) fn poll_pending_prepare(&mut self, ctx: &egui::Context) {
        let poll = self
            .pending_prepare
            .as_ref()
            .map(PendingPrepareUnlock::poll);

        match poll {
            Some(PrepareUnlockPoll::Pending) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            Some(PrepareUnlockPoll::Finished(Ok(prepared))) => {
                self.pending_prepare = None;
                match begin_unlock_verification(prepared, &self.owner_window) {
                    Ok(pending_unlock) => {
                        self.state.loader.error = None;
                        self.pending_unlock = Some(pending_unlock);
                        self.set_banner(
                            BannerTone::Info,
                            tr("Correct password. Waiting for Windows verification to open the vault."),
                        );
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    }
                    Err(error) => {
                        self.state.banner = None;
                        self.state.loader.error = Some(error);
                    }
                }
            }
            Some(PrepareUnlockPoll::Finished(Err(error))) => {
                self.pending_prepare = None;
                self.state.banner = None;
                self.state.loader.error = Some(error);
            }
            None => {}
        }
    }

    pub(crate) fn poll_pending_unlock(&mut self, ctx: &egui::Context) {
        let poll = self.pending_unlock.as_ref().map(PendingUnlockFlow::poll);

        match poll {
            Some(VerificationPollResult::Pending) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            Some(VerificationPollResult::Finished(Ok(()))) => {
                if let Some(pending) = self.pending_unlock.take() {
                    let (password, session) = pending.into_parts();
                    self.vault.finish_unlock(password, session);
                    self.state.loader.error = None;
                    self.state.screen = crate::state::Screen::Main;
                    self.sync_selection();
                    self.state.banner = None;
                    self.state.notice_dialog.show(
                        tr("Vault unlocked"),
                        tr("Vault unlocked. Access was validated by your master password and by the operating system."),
                    );
                }
            }
            Some(VerificationPollResult::Finished(Err(error))) => {
                self.pending_unlock = None;
                self.state.banner = None;
                self.state.loader.error = Some(error);
            }
            None => {}
        }
    }
}
