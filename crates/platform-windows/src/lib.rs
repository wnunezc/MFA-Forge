use mfa_forge_application::ports::{
    UserPresenceVerifier, VerificationHandle, VerificationPollResult,
};
use raw_window_handle::HasWindowHandle;
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub struct OwnerWindow {
    #[cfg(target_os = "windows")]
    hwnd: usize,
}

pub fn capture_owner_window(source: &impl HasWindowHandle) -> Result<OwnerWindow, String> {
    platform::capture_owner_window(source)
}

/// Configures per-monitor DPI awareness before the GUI creates its native window.
pub fn configure_process_dpi_awareness() -> Result<(), String> {
    platform::configure_process_dpi_awareness()
}

/// Restores the main window using physical Win32 coordinates or maximizes it on first launch.
pub fn initialize_main_window(owner_window: OwnerWindow, state_path: &Path) -> Result<(), String> {
    platform::initialize_main_window(owner_window, state_path)
}

/// Saves the main window monitor, restored bounds and maximized state.
pub fn save_main_window(owner_window: OwnerWindow, state_path: &Path) -> Result<(), String> {
    platform::save_main_window(owner_window, state_path)
}

pub fn begin_verify_unlock(owner_window: OwnerWindow) -> Result<PendingVerification, String> {
    platform::begin_verify_unlock(owner_window)
}

pub fn settle_closed_prompt_window() {
    platform::settle_closed_prompt_window()
}

/// Drains pending Win32 messages owned by the current thread.
pub fn pump_pending_messages() {
    platform::pump_pending_messages();
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

fn wait_with_timeout<T>(
    timeout: Duration,
    poll_interval: Duration,
    mut poll: impl FnMut() -> Result<Option<T>, String>,
    mut cancel: impl FnMut(),
    timeout_message: &str,
) -> Result<T, String> {
    let started_at = Instant::now();
    loop {
        if let Some(value) = poll()? {
            return Ok(value);
        }
        if started_at.elapsed() >= timeout {
            cancel();
            return Err(timeout_message.to_owned());
        }
        std::thread::sleep(poll_interval);
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::{
        fs, mem::size_of, panic::AssertUnwindSafe, path::Path, sync::mpsc, thread, time::Duration,
    };

    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use serde::{Deserialize, Serialize};
    use windows::{
        Security::Credentials::UI::{
            UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
        },
        Win32::{
            Foundation::{HWND, LPARAM, RECT},
            Graphics::Gdi::{
                EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTOPRIMARY,
                MONITORINFOEXW, MonitorFromWindow,
            },
            System::WinRT::{
                IUserConsentVerifierInterop, RO_INIT_MULTITHREADED, RoGetActivationFactory,
                RoInitialize, RoUninitialize,
            },
            UI::{
                HiDpi::{
                    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, GetDpiForWindow,
                    MDT_EFFECTIVE_DPI, SetProcessDpiAwarenessContext, SetThreadDpiAwarenessContext,
                },
                WindowsAndMessaging::{
                    BringWindowToTop, GetWindowPlacement, HWND_TOP, IsIconic, IsWindow,
                    SW_MAXIMIZE, SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow, SetWindowPos,
                    ShowWindow, ShowWindowAsync, WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS,
                },
            },
        },
        core::{BOOL, HSTRING},
    };
    use windows_future::{AsyncStatus, IAsyncOperation};

    const USER_CONSENT_CLASS: &str = "Windows.Security.Credentials.UI.UserConsentVerifier";
    const UNLOCK_REASON: &str =
        "Confirma tu identidad con Windows para desbloquear el vault de MFA-Forge.";
    const WINDOWS_ASYNC_TIMEOUT: Duration = Duration::from_secs(120);
    const WINDOWS_ASYNC_POLL_INTERVAL: Duration = Duration::from_millis(50);
    const WINDOW_STATE_VERSION: u32 = 1;
    const DEFAULT_MAIN_WINDOW_WIDTH: i32 = 1380;
    const DEFAULT_MAIN_WINDOW_HEIGHT: i32 = 860;
    const MIN_RESTORED_WINDOW_WIDTH: i32 = 960;
    const MIN_RESTORED_WINDOW_HEIGHT: i32 = 640;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct MainWindowState {
        version: u32,
        monitor: String,
        offset_x: i32,
        offset_y: i32,
        width: i32,
        height: i32,
        maximized: bool,
    }

    #[derive(Clone)]
    struct MonitorDetails {
        device: String,
        work: RECT,
    }

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

    pub fn configure_process_dpi_awareness() -> Result<(), String> {
        let _ =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        Ok(())
    }

    pub fn initialize_main_window(
        owner_window: super::OwnerWindow,
        state_path: &Path,
    ) -> Result<(), String> {
        let hwnd = checked_hwnd(owner_window)?;
        let restored = fs::read_to_string(state_path)
            .ok()
            .and_then(|json| serde_json::from_str::<MainWindowState>(&json).ok())
            .filter(|state| state.version == WINDOW_STATE_VERSION)
            .and_then(|state| restore_window_state(hwnd, &state).ok())
            .is_some();

        if !restored {
            let primary =
                monitor_details(unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) })?;
            let width = DEFAULT_MAIN_WINDOW_WIDTH.min(primary.work.right - primary.work.left);
            let height = DEFAULT_MAIN_WINDOW_HEIGHT.min(primary.work.bottom - primary.work.top);
            let x = primary.work.left + ((primary.work.right - primary.work.left - width) / 2);
            let y = primary.work.top + ((primary.work.bottom - primary.work.top - height) / 2);
            unsafe {
                SetWindowPos(
                    hwnd,
                    Some(HWND_TOP),
                    x,
                    y,
                    width,
                    height,
                    Default::default(),
                )
                .map_err(|error| format!("No se pudo posicionar la ventana principal: {error}"))?;
                let _ = ShowWindow(hwnd, SW_MAXIMIZE);
            }
        }

        Ok(())
    }

    pub fn save_main_window(
        owner_window: super::OwnerWindow,
        state_path: &Path,
    ) -> Result<(), String> {
        let _dpi_guard = DpiAwarenessGuard::per_monitor_v2();
        let hwnd = checked_hwnd(owner_window)?;
        let mut placement = WINDOWPLACEMENT {
            length: size_of::<WINDOWPLACEMENT>() as u32,
            flags: WINDOWPLACEMENT_FLAGS(0),
            ..Default::default()
        };
        unsafe { GetWindowPlacement(hwnd, &mut placement) }
            .map_err(|error| format!("No se pudo leer la posición de la ventana: {error}"))?;

        let monitor =
            monitor_details(unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) })?;
        let rect = placement.rcNormalPosition;
        let raw_state = MainWindowState {
            version: WINDOW_STATE_VERSION,
            monitor: monitor.device,
            offset_x: rect.left - monitor.work.left,
            offset_y: rect.top - monitor.work.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
            maximized: placement.showCmd == SW_MAXIMIZE.0 as u32,
        };
        let (x, y, width, height) = clamped_window_rect(&monitor.work, &raw_state);
        let mut state = MainWindowState {
            offset_x: x - monitor.work.left,
            offset_y: y - monitor.work.top,
            width,
            height,
            ..raw_state
        };
        if let Some(previous) = fs::read_to_string(state_path)
            .ok()
            .and_then(|json| serde_json::from_str::<MainWindowState>(&json).ok())
        {
            preserve_stable_restored_size(&previous, &mut state);
        }
        let json = serde_json::to_string_pretty(&state)
            .map_err(|error| format!("No se pudo serializar el estado de ventana: {error}"))?;
        if fs::read_to_string(state_path)
            .map(|existing_json| existing_json == json)
            .unwrap_or(false)
        {
            return Ok(());
        }
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("No se pudo crear el directorio de estado de ventana: {error}")
            })?;
        }
        fs::write(state_path, json)
            .map_err(|error| format!("No se pudo guardar el estado de ventana: {error}"))
    }

    fn checked_hwnd(owner_window: super::OwnerWindow) -> Result<HWND, String> {
        let hwnd = HWND(owner_window.hwnd as *mut _);
        if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            Ok(hwnd)
        } else {
            Err("La ventana principal de MFA-Forge ya no es válida.".to_owned())
        }
    }

    fn restore_window_state(hwnd: HWND, state: &MainWindowState) -> Result<(), String> {
        let _dpi_guard = DpiAwarenessGuard::per_monitor_v2();
        let monitor = match find_monitor(&state.monitor) {
            Some(monitor) => monitor,
            None if state.maximized => {
                monitor_details(unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) })?
            }
            None => {
                return Err("El monitor guardado ya no está disponible.".to_owned());
            }
        };
        let (x, y, width, height) = clamped_window_rect(&monitor.work, state);
        let (width, height) =
            compensate_restore_size_for_current_dpi(hwnd, &monitor, width, height);

        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                x,
                y,
                width,
                height,
                Default::default(),
            )
            .map_err(|error| format!("No se pudo restaurar la ventana principal: {error}"))?;
            let _ = ShowWindow(
                hwnd,
                if state.maximized {
                    SW_MAXIMIZE
                } else {
                    SW_SHOWNORMAL
                },
            );
        }
        Ok(())
    }

    struct DpiAwarenessGuard {
        previous: windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT,
    }

    impl DpiAwarenessGuard {
        fn per_monitor_v2() -> Self {
            let previous =
                unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
            Self { previous }
        }
    }

    impl Drop for DpiAwarenessGuard {
        fn drop(&mut self) {
            let _ = unsafe { SetThreadDpiAwarenessContext(self.previous) };
        }
    }

    fn clamped_window_rect(work: &RECT, state: &MainWindowState) -> (i32, i32, i32, i32) {
        let work_width = work.right - work.left;
        let work_height = work.bottom - work.top;
        let width = state
            .width
            .clamp(MIN_RESTORED_WINDOW_WIDTH.min(work_width), work_width);
        let height = state
            .height
            .clamp(MIN_RESTORED_WINDOW_HEIGHT.min(work_height), work_height);
        let x = (work.left + state.offset_x).clamp(work.left, work.right - width);
        let y = (work.top + state.offset_y).clamp(work.top, work.bottom - height);
        (x, y, width, height)
    }

    fn preserve_stable_restored_size(previous: &MainWindowState, current: &mut MainWindowState) {
        if previous.version == current.version
            && previous.monitor == current.monitor
            && !previous.maximized
            && !current.maximized
            && (previous.width - current.width).abs() <= 4
            && (previous.height - current.height).abs() <= 4
        {
            current.width = previous.width;
            current.height = previous.height;
        }
    }

    fn compensate_restore_size_for_current_dpi(
        hwnd: HWND,
        monitor: &MonitorDetails,
        width: i32,
        height: i32,
    ) -> (i32, i32) {
        let current_dpi = unsafe { GetDpiForWindow(hwnd) };
        let target_dpi = monitor_dpi(monitor).unwrap_or(current_dpi);
        if current_dpi == 0 || target_dpi == 0 || current_dpi == target_dpi {
            return (width, height);
        }

        (
            scale_i32(width, current_dpi, target_dpi),
            scale_i32(height, current_dpi, target_dpi),
        )
    }

    fn monitor_dpi(monitor: &MonitorDetails) -> Option<u32> {
        let handle = find_monitor_handle(&monitor.device)?;
        let mut dpi_x = 0;
        let mut dpi_y = 0;
        unsafe { GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.ok()?;
        Some(dpi_x)
    }

    fn scale_i32(value: i32, numerator: u32, denominator: u32) -> i32 {
        ((i64::from(value) * i64::from(numerator) + i64::from(denominator / 2))
            / i64::from(denominator)) as i32
    }

    fn find_monitor(device: &str) -> Option<MonitorDetails> {
        unsafe extern "system" fn callback(
            monitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            data: LPARAM,
        ) -> BOOL {
            let context = unsafe { &mut *(data.0 as *mut (&str, Option<MonitorDetails>)) };
            if let Ok(details) = monitor_details(monitor)
                && details.device == context.0
            {
                context.1 = Some(details);
                return false.into();
            }
            true.into()
        }

        let mut context = (device, None);
        unsafe {
            let _ = EnumDisplayMonitors(
                None,
                None,
                Some(callback),
                LPARAM((&mut context as *mut (&str, Option<MonitorDetails>)) as isize),
            );
        }
        context.1
    }

    fn monitor_details(handle: HMONITOR) -> Result<MonitorDetails, String> {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if !unsafe { GetMonitorInfoW(handle, &mut info.monitorInfo as *mut _) }.as_bool() {
            return Err("No se pudo consultar el monitor de la ventana.".to_owned());
        }
        let device_len = info
            .szDevice
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(info.szDevice.len());
        Ok(MonitorDetails {
            device: String::from_utf16_lossy(&info.szDevice[..device_len]),
            work: info.monitorInfo.rcWork,
        })
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

        let started_at = Instant::now();
        while started_at.elapsed() < Duration::from_millis(250) {
            let drained_any = pump_pending_messages();

            if !drained_any {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    /// Drains all currently queued Win32 messages and reports whether any were dispatched.
    pub fn pump_pending_messages() -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
        };

        let mut drained_any = false;
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).into() {
                drained_any = true;
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        drained_any
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
        let availability = wait_for(
            UserConsentVerifier::CheckAvailabilityAsync().map_err(map_windows_error)?,
            "Windows Hello no respondió dentro de 120 segundos al comprobar su disponibilidad.",
        )?;

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

        match wait_for(
            verification,
            "Windows Hello no respondió dentro de 120 segundos durante la verificación.",
        )? {
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

    fn wait_for<T>(operation: IAsyncOperation<T>, timeout_message: &str) -> Result<T, String>
    where
        T: windows::core::RuntimeType,
    {
        let result = super::wait_with_timeout(
            WINDOWS_ASYNC_TIMEOUT,
            WINDOWS_ASYNC_POLL_INTERVAL,
            || match operation.Status().map_err(map_windows_error)? {
                AsyncStatus::Started => Ok(None),
                AsyncStatus::Completed => {
                    operation.GetResults().map(Some).map_err(map_windows_error)
                }
                AsyncStatus::Canceled => Err("La operación de Windows fue cancelada.".to_owned()),
                AsyncStatus::Error => operation.GetResults().map(Some).map_err(map_windows_error),
                _ => Err("Windows devolvió un estado asíncrono desconocido.".to_owned()),
            },
            || {
                let _ = operation.Cancel();
                let _ = operation.Close();
            },
            timeout_message,
        );

        if result.is_ok() {
            let _ = operation.Close();
        }
        result
    }

    fn find_monitor_handle(device: &str) -> Option<HMONITOR> {
        unsafe extern "system" fn callback(
            monitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            data: LPARAM,
        ) -> BOOL {
            let context = unsafe { &mut *(data.0 as *mut (&str, Option<HMONITOR>)) };
            if let Ok(details) = monitor_details(monitor)
                && details.device == context.0
            {
                context.1 = Some(monitor);
                return false.into();
            }
            true.into()
        }

        let mut context = (device, None);
        unsafe {
            let _ = EnumDisplayMonitors(
                None,
                None,
                Some(callback),
                LPARAM(&mut context as *mut _ as isize),
            );
        }
        context.1
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

    #[cfg(test)]
    mod window_state_tests {
        use super::*;

        #[test]
        fn restored_bounds_stay_inside_negative_origin_monitor() {
            let work = RECT {
                left: -1920,
                top: 0,
                right: 0,
                bottom: 1032,
            };
            let state = MainWindowState {
                version: WINDOW_STATE_VERSION,
                monitor: r"\\.\DISPLAY5".to_owned(),
                offset_x: 220,
                offset_y: 100,
                width: 1100,
                height: 760,
                maximized: false,
            };

            assert_eq!(clamped_window_rect(&work, &state), (-1700, 100, 1100, 760));
        }

        #[test]
        fn oversized_restored_bounds_are_clamped_to_work_area() {
            let work = RECT {
                left: 0,
                top: 0,
                right: 1536,
                bottom: 816,
            };
            let state = MainWindowState {
                version: WINDOW_STATE_VERSION,
                monitor: r"\\.\DISPLAY1".to_owned(),
                offset_x: 500,
                offset_y: 300,
                width: 3000,
                height: 2000,
                maximized: true,
            };

            assert_eq!(clamped_window_rect(&work, &state), (0, 0, 1536, 816));
        }

        #[test]
        fn undersized_restored_bounds_are_expanded_before_restore() {
            let work = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1032,
            };
            let state = MainWindowState {
                version: WINDOW_STATE_VERSION,
                monitor: r"\\.\DISPLAY1".to_owned(),
                offset_x: 823,
                offset_y: 250,
                width: 883,
                height: 600,
                maximized: false,
            };

            assert_eq!(
                clamped_window_rect(&work, &state),
                (
                    823,
                    250,
                    MIN_RESTORED_WINDOW_WIDTH,
                    MIN_RESTORED_WINDOW_HEIGHT
                )
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use raw_window_handle::HasWindowHandle;
    use std::path::Path;

    pub fn capture_owner_window(
        _source: &impl HasWindowHandle,
    ) -> Result<super::OwnerWindow, String> {
        Ok(super::OwnerWindow {})
    }

    pub fn configure_process_dpi_awareness() -> Result<(), String> {
        Ok(())
    }

    pub fn initialize_main_window(
        _owner_window: super::OwnerWindow,
        _state_path: &Path,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn save_main_window(
        _owner_window: super::OwnerWindow,
        _state_path: &Path,
    ) -> Result<(), String> {
        Ok(())
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

    /// No-op implementation for unsupported platforms.
    pub fn pump_pending_messages() {}
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, time::Duration};

    use super::wait_with_timeout;

    #[test]
    fn bounded_wait_returns_completed_value_without_canceling() {
        let canceled = Cell::new(false);

        let result = wait_with_timeout(
            Duration::from_millis(50),
            Duration::from_millis(1),
            || Ok(Some(42)),
            || canceled.set(true),
            "timed out",
        )
        .expect("operation should complete");

        assert_eq!(result, 42);
        assert!(!canceled.get());
    }

    #[test]
    fn bounded_wait_cancels_when_deadline_expires() {
        let canceled = Cell::new(false);

        let error = wait_with_timeout::<()>(
            Duration::from_millis(5),
            Duration::from_millis(1),
            || Ok(None),
            || canceled.set(true),
            "synthetic timeout",
        )
        .expect_err("operation should time out");

        assert_eq!(error, "synthetic timeout");
        assert!(canceled.get());
    }
}
