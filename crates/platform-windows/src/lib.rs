use mfa_forge_application::ports::{
    UserPresenceVerifier, VerificationHandle, VerificationPollResult,
};
use raw_window_handle::HasWindowHandle;

#[derive(Clone, Copy, Debug)]
pub struct OwnerWindow {
    #[cfg(target_os = "windows")]
    hwnd: usize,
}

pub fn capture_owner_window(source: &impl HasWindowHandle) -> Result<OwnerWindow, String> {
    platform::capture_owner_window(source)
}

pub fn begin_verify_unlock(owner_window: OwnerWindow) -> Result<PendingVerification, String> {
    platform::begin_verify_unlock(owner_window)
}

pub fn settle_closed_prompt_window() {
    platform::settle_closed_prompt_window()
}

pub enum VerificationPoll {
    Pending,
    Finished(Result<(), String>),
}

pub struct PendingVerification {
    inner: platform::PlatformPendingVerification,
}

impl PendingVerification {
    pub fn poll(&self) -> VerificationPoll {
        self.inner.poll()
    }
}

impl VerificationHandle for PendingVerification {
    fn poll_verification(&self) -> VerificationPollResult {
        match self.poll() {
            VerificationPoll::Pending => VerificationPollResult::Pending,
            VerificationPoll::Finished(result) => VerificationPollResult::Finished(result),
        }
    }
}

impl UserPresenceVerifier for OwnerWindow {
    type PendingVerification = PendingVerification;

    fn begin_verification(&self) -> Result<Self::PendingVerification, String> {
        begin_verify_unlock(*self)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::{panic::AssertUnwindSafe, sync::mpsc, thread};

    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::{
        Security::Credentials::UI::{
            UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
        },
        Win32::{
            Foundation::HWND,
            System::WinRT::{
                IUserConsentVerifierInterop, RO_INIT_MULTITHREADED, RoGetActivationFactory,
                RoInitialize, RoUninitialize,
            },
            UI::WindowsAndMessaging::{
                BringWindowToTop, IsIconic, IsWindow, SW_RESTORE, SetForegroundWindow,
                ShowWindowAsync,
            },
        },
        core::HSTRING,
    };
    use windows_future::IAsyncOperation;

    const USER_CONSENT_CLASS: &str = "Windows.Security.Credentials.UI.UserConsentVerifier";
    const UNLOCK_REASON: &str =
        "Confirma tu identidad con Windows para desbloquear el vault de MFA-Forge.";

    pub struct PlatformPendingVerification {
        receiver: mpsc::Receiver<Result<(), String>>,
    }

    impl PlatformPendingVerification {
        pub fn poll(&self) -> super::VerificationPoll {
            match self.receiver.try_recv() {
                Ok(result) => super::VerificationPoll::Finished(result),
                Err(mpsc::TryRecvError::Empty) => super::VerificationPoll::Pending,
                Err(mpsc::TryRecvError::Disconnected) => super::VerificationPoll::Finished(Err(
                    "La verificación de Windows terminó de forma inesperada.".to_owned(),
                )),
            }
        }
    }

    pub fn capture_owner_window(
        source: &impl HasWindowHandle,
    ) -> Result<super::OwnerWindow, String> {
        let raw_window = source.window_handle().map_err(|error| {
            format!("No se pudo obtener el handle nativo de la ventana de MFA-Forge: {error}")
        })?;

        match raw_window.as_raw() {
            RawWindowHandle::Win32(handle) => {
                let hwnd = handle.hwnd.get();
                if hwnd == 0 {
                    return Err(
                        "La ventana de MFA-Forge no expuso un HWND válido para la verificación de Windows."
                            .to_owned(),
                    );
                }

                Ok(super::OwnerWindow {
                    hwnd: hwnd as usize,
                })
            }
            _ => Err(
                "La verificación adicional de Windows requiere una ventana Win32 válida."
                    .to_owned(),
            ),
        }
    }

    pub fn begin_verify_unlock(
        owner_window: super::OwnerWindow,
    ) -> Result<super::PendingVerification, String> {
        let hwnd = HWND(owner_window.hwnd as *mut _);
        prepare_owner_window(hwnd)?;
        let owner_hwnd = owner_window.hwnd;
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let result =
                std::panic::catch_unwind(AssertUnwindSafe(|| verify_unlock_worker(owner_hwnd)))
                    .unwrap_or_else(|panic| {
                        Err(format!(
                            "La verificación de Windows terminó por panic: {}",
                            panic_message(panic)
                        ))
                    });
            let _ = sender.send(result);
        });

        Ok(super::PendingVerification {
            inner: PlatformPendingVerification { receiver },
        })
    }

    pub fn settle_closed_prompt_window() {
        use std::time::{Duration, Instant};
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
        };

        let started_at = Instant::now();
        while started_at.elapsed() < Duration::from_millis(250) {
            let mut drained_any = false;
            unsafe {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).into() {
                    drained_any = true;
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }

            if !drained_any {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn prepare_owner_window(hwnd: HWND) -> Result<(), String> {
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err(
                "No se pudo asociar la validación del sistema con una ventana Win32 válida de MFA-Forge."
                    .to_owned(),
            );
        }

        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            }
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
        }

        Ok(())
    }

    fn verify_unlock_worker(hwnd: usize) -> Result<(), String> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(map_windows_error)?;

        let outcome = verify_unlock_inner(HWND(hwnd as *mut _));

        unsafe { RoUninitialize() };

        outcome
    }

    fn verify_unlock_inner(hwnd: HWND) -> Result<(), String> {
        let availability =
            wait_for(UserConsentVerifier::CheckAvailabilityAsync().map_err(map_windows_error)?)
                .map_err(map_windows_error)?;

        match availability {
            UserConsentVerifierAvailability::Available => {}
            UserConsentVerifierAvailability::DeviceBusy => {
                return Err(
                    "La validación de Windows está ocupada. Intenta desbloquear nuevamente en unos segundos."
                        .to_owned(),
                );
            }
            UserConsentVerifierAvailability::DeviceNotPresent => {
                return Err(
                    "Windows no encontró un método local de verificación disponible para reforzar el desbloqueo."
                        .to_owned(),
                );
            }
            UserConsentVerifierAvailability::NotConfiguredForUser => {
                return Err(
                    "Configura Windows Hello o PIN en tu sesión antes de desbloquear el vault desde la GUI."
                        .to_owned(),
                );
            }
            UserConsentVerifierAvailability::DisabledByPolicy => {
                return Err(
                    "La verificación adicional de Windows está deshabilitada por política del sistema."
                        .to_owned(),
                );
            }
            _ => {
                return Err(
                    "Windows no pudo ofrecer una validación adicional para este desbloqueo."
                        .to_owned(),
                );
            }
        }

        let interop = unsafe {
            RoGetActivationFactory::<IUserConsentVerifierInterop>(&HSTRING::from(
                USER_CONSENT_CLASS,
            ))
        }
        .map_err(map_windows_error)?;

        let verification = unsafe {
            interop.RequestVerificationForWindowAsync::<IAsyncOperation<UserConsentVerificationResult>>(
                hwnd,
                &HSTRING::from(UNLOCK_REASON),
            )
        }
        .map_err(map_windows_error)?;

        match wait_for(verification).map_err(map_windows_error)? {
            UserConsentVerificationResult::Verified => Ok(()),
            UserConsentVerificationResult::Canceled => {
                Err("La validación del sistema fue cancelada.".to_owned())
            }
            UserConsentVerificationResult::RetriesExhausted => Err(
                "Windows bloqueó temporalmente la validación por demasiados intentos fallidos."
                    .to_owned(),
            ),
            UserConsentVerificationResult::DeviceBusy => Err(
                "El método de validación de Windows está ocupado. Intenta nuevamente.".to_owned(),
            ),
            UserConsentVerificationResult::DeviceNotPresent => {
                Err("Windows no encontró un método local de verificación disponible.".to_owned())
            }
            UserConsentVerificationResult::NotConfiguredForUser => Err(
                "Configura Windows Hello o PIN para usar la protección adicional del desbloqueo."
                    .to_owned(),
            ),
            UserConsentVerificationResult::DisabledByPolicy => Err(
                "La política del sistema deshabilita la verificación adicional de Windows."
                    .to_owned(),
            ),
            _ => Err(
                "La validación adicional de Windows no pudo completarse en este equipo.".to_owned(),
            ),
        }
    }

    fn wait_for<T>(operation: IAsyncOperation<T>) -> windows::core::Result<T>
    where
        T: windows::core::RuntimeType,
    {
        operation.join()
    }

    fn map_windows_error(error: windows::core::Error) -> String {
        let message = error.message();
        if message.is_empty() {
            "Windows no pudo completar la validación adicional requerida.".to_owned()
        } else {
            format!("Windows rechazó la validación adicional: {message}")
        }
    }

    fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
        match payload.downcast::<String>() {
            Ok(message) => *message,
            Err(payload) => match payload.downcast::<&'static str>() {
                Ok(message) => (*message).to_owned(),
                Err(_) => "panic without string payload".to_owned(),
            },
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use raw_window_handle::HasWindowHandle;

    pub fn capture_owner_window(
        _source: &impl HasWindowHandle,
    ) -> Result<super::OwnerWindow, String> {
        Ok(super::OwnerWindow {})
    }

    pub struct PlatformPendingVerification;

    impl PlatformPendingVerification {
        pub fn poll(&self) -> super::VerificationPoll {
            super::VerificationPoll::Finished(Err(
                "La validación adicional del sistema operativo solo está implementada para Windows en esta RC.".to_owned(),
            ))
        }
    }

    pub fn begin_verify_unlock(
        _owner_window: super::OwnerWindow,
    ) -> Result<super::PendingVerification, String> {
        Ok(super::PendingVerification {
            inner: PlatformPendingVerification,
        })
    }

    pub fn settle_closed_prompt_window() {}
}
