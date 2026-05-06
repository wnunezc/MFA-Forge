use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use eframe::egui::{self, Align, Color32, Layout, RichText, TextEdit};
use mfa_forge_application::{
    ports::VerificationPollResult,
    unlock::{
        PendingPrepareUnlock, PendingUnlockFlow, PrepareUnlockPoll, begin_unlock_verification,
        spawn_prepare_unlock,
    },
};
use secrecy::SecretString;
use serde_json::json;
use zeroize::Zeroize;

use mfa_forge_core::AccountPublic;

use crate::{
    diagnostics, platform_auth,
    theme::{self, ThemePreference},
    vault::{PendingUnlockSession, VaultFacade},
};

type AgentPendingPrepareUnlock = PendingPrepareUnlock<PendingUnlockSession>;
type AgentPendingUnlockFlow =
    PendingUnlockFlow<PendingUnlockSession, platform_auth::PendingVerification>;

fn ui_trace(event: &str, details: impl AsRef<str>) {
    diagnostics::log_event("ui", event, json!({ "details": details.as_ref() }));
    if std::env::var_os("MFA_FORGE_UI_TRACE").is_some() {
        eprintln!("[mfa-forge-ui-trace] {event} {}", details.as_ref());
    }
}

fn trace_frame_sample(
    event: &str,
    ctx: &egui::Context,
    opened_at: Instant,
    last_frame_sample_second: &mut Option<u64>,
) {
    if !diagnostics::trace_enabled() {
        return;
    }

    let elapsed_seconds = opened_at.elapsed().as_secs();
    if elapsed_seconds > 10 || *last_frame_sample_second == Some(elapsed_seconds) {
        return;
    }

    let screen_rect = ctx.screen_rect();
    ui_trace(
        event,
        format!(
            "elapsed_s={} screen_width={:.1} screen_height={:.1}",
            elapsed_seconds,
            screen_rect.width(),
            screen_rect.height()
        ),
    );
    *last_frame_sample_second = Some(elapsed_seconds);
}

fn request_transient_repaint(ctx: &egui::Context, opened_at: Instant) {
    if opened_at.elapsed() < Duration::from_secs(15) {
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

pub fn run_unlock_window() -> Result<VaultFacade, String> {
    let theme_preference = theme::load_preference();
    let outcome = Rc::new(RefCell::new(None));
    let app_icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icons/app-icon.png"))
            .map_err(|error| error.to_string())?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MFA-Forge Agent Session")
            .with_inner_size([520.0, 260.0])
            .with_min_inner_size([520.0, 260.0])
            .with_resizable(false)
            .with_icon(app_icon),
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "MFA-Forge Agent Session",
        options,
        Box::new({
            let outcome = Rc::clone(&outcome);
            move |cc| {
                AgentUnlockApp::new(cc, theme_preference, Rc::clone(&outcome))
                    .map(|app| Box::new(app) as Box<dyn eframe::App>)
                    .map_err(|error| error.into())
            }
        }),
    )
    .map_err(|error| error.to_string())?;

    platform_auth::settle_closed_prompt_window();

    outcome.borrow_mut().take().unwrap_or_else(|| {
        Err("El acceso fue cancelado antes de abrir la sesión de agente.".to_owned())
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenGrantPromptDecision {
    Approved,
    Denied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningGrantPromptDecision {
    Approved,
    Denied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditReportingGrantPromptDecision {
    Approved,
    Denied,
}

#[derive(Debug)]
pub enum PasswordRotationPromptDecision {
    Approved { new_password: SecretString },
    Denied,
}

pub fn run_generate_token_grant_window(
    account: &AccountPublic,
    ttl_seconds: u64,
) -> Result<TokenGrantPromptDecision, String> {
    ui_trace(
        "token_grant.start",
        format!("ttl_seconds={ttl_seconds} account_id={}", account.id),
    );
    let theme_preference = theme::load_preference();
    let outcome = Rc::new(RefCell::new(None));
    let app_icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icons/app-icon.png"))
            .map_err(|error| error.to_string())?;
    let account = account.clone();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MFA-Forge Token Grant")
            .with_inner_size([760.0, 360.0])
            .with_min_inner_size([640.0, 300.0])
            .with_clamp_size_to_monitor_size(true)
            .with_resizable(true)
            .with_icon(app_icon),
        persist_window: false,
        ..Default::default()
    };

    let run_result = eframe::run_native(
        "mfa-forge-token-grant-prompt",
        options,
        Box::new({
            let outcome = Rc::clone(&outcome);
            move |cc| {
                Ok(Box::new(TokenGrantApprovalApp::new(
                    cc,
                    theme_preference,
                    account.clone(),
                    ttl_seconds,
                    Rc::clone(&outcome),
                )) as Box<dyn eframe::App>)
            }
        }),
    );
    run_result.map_err(|error| error.to_string())?;

    let decision = outcome
        .borrow_mut()
        .take()
        .unwrap_or(TokenGrantPromptDecision::Denied);
    ui_trace("token_grant.end", format!("decision={decision:?}"));
    Ok(decision)
}

pub fn run_account_provisioning_grant_window(
    account_limit: u8,
    ttl_minutes: u64,
) -> Result<ProvisioningGrantPromptDecision, String> {
    ui_trace(
        "provisioning_grant.start",
        format!("account_limit={account_limit} ttl_minutes={ttl_minutes}"),
    );
    let theme_preference = theme::load_preference();
    let outcome = Rc::new(RefCell::new(None));
    let app_icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icons/app-icon.png"))
            .map_err(|error| error.to_string())?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MFA-Forge Provisioning Grant")
            .with_inner_size([760.0, 380.0])
            .with_min_inner_size([640.0, 320.0])
            .with_clamp_size_to_monitor_size(true)
            .with_resizable(true)
            .with_icon(app_icon),
        persist_window: false,
        ..Default::default()
    };

    let run_result = eframe::run_native(
        "mfa-forge-provisioning-grant-prompt",
        options,
        Box::new({
            let outcome = Rc::clone(&outcome);
            move |cc| {
                Ok(Box::new(ProvisioningGrantApprovalApp::new(
                    cc,
                    theme_preference,
                    account_limit,
                    ttl_minutes,
                    Rc::clone(&outcome),
                )) as Box<dyn eframe::App>)
            }
        }),
    );
    run_result.map_err(|error| error.to_string())?;

    let decision = outcome
        .borrow_mut()
        .take()
        .unwrap_or(ProvisioningGrantPromptDecision::Denied);
    ui_trace("provisioning_grant.end", format!("decision={decision:?}"));
    Ok(decision)
}

pub fn run_audit_reporting_grant_window(
    read_limit: u8,
    ttl_minutes: u64,
) -> Result<AuditReportingGrantPromptDecision, String> {
    ui_trace(
        "audit_reporting_grant.start",
        format!("read_limit={read_limit} ttl_minutes={ttl_minutes}"),
    );
    let theme_preference = theme::load_preference();
    let outcome = Rc::new(RefCell::new(None));
    let app_icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icons/app-icon.png"))
            .map_err(|error| error.to_string())?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MFA-Forge Audit Reporting Grant")
            .with_inner_size([760.0, 380.0])
            .with_min_inner_size([640.0, 320.0])
            .with_clamp_size_to_monitor_size(true)
            .with_resizable(true)
            .with_icon(app_icon),
        persist_window: false,
        ..Default::default()
    };

    let run_result = eframe::run_native(
        "mfa-forge-audit-reporting-grant-prompt",
        options,
        Box::new({
            let outcome = Rc::clone(&outcome);
            move |cc| {
                Ok(Box::new(AuditReportingGrantApprovalApp::new(
                    cc,
                    theme_preference,
                    read_limit,
                    ttl_minutes,
                    Rc::clone(&outcome),
                )) as Box<dyn eframe::App>)
            }
        }),
    );
    run_result.map_err(|error| error.to_string())?;

    let decision = outcome
        .borrow_mut()
        .take()
        .unwrap_or(AuditReportingGrantPromptDecision::Denied);
    ui_trace(
        "audit_reporting_grant.end",
        format!("decision={decision:?}"),
    );
    Ok(decision)
}

pub fn run_password_rotation_window() -> Result<PasswordRotationPromptDecision, String> {
    ui_trace("password_rotation.start", "prompt=opened");
    let theme_preference = theme::load_preference();
    let outcome = Rc::new(RefCell::new(None));
    let app_icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icons/app-icon.png"))
            .map_err(|error| error.to_string())?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MFA-Forge Password Rotation")
            .with_inner_size([760.0, 420.0])
            .with_min_inner_size([640.0, 360.0])
            .with_clamp_size_to_monitor_size(true)
            .with_resizable(true)
            .with_icon(app_icon),
        persist_window: false,
        ..Default::default()
    };

    let run_result = eframe::run_native(
        "mfa-forge-password-rotation-prompt",
        options,
        Box::new({
            let outcome = Rc::clone(&outcome);
            move |cc| {
                Ok(Box::new(PasswordRotationApprovalApp::new(
                    cc,
                    theme_preference,
                    Rc::clone(&outcome),
                )) as Box<dyn eframe::App>)
            }
        }),
    );
    run_result.map_err(|error| error.to_string())?;

    let decision = outcome
        .borrow_mut()
        .take()
        .unwrap_or(PasswordRotationPromptDecision::Denied);
    ui_trace(
        "password_rotation.end",
        format!(
            "decision={}",
            match decision {
                PasswordRotationPromptDecision::Approved { .. } => "approved",
                PasswordRotationPromptDecision::Denied => "denied",
            }
        ),
    );
    Ok(decision)
}

struct TokenGrantApprovalApp {
    theme_preference: ThemePreference,
    account: AccountPublic,
    ttl_seconds: u64,
    outcome: Rc<RefCell<Option<TokenGrantPromptDecision>>>,
    first_frame_logged: bool,
    opened_at: Instant,
    last_frame_sample_second: Option<u64>,
}

struct ProvisioningGrantApprovalApp {
    theme_preference: ThemePreference,
    account_limit: u8,
    ttl_minutes: u64,
    outcome: Rc<RefCell<Option<ProvisioningGrantPromptDecision>>>,
    first_frame_logged: bool,
    opened_at: Instant,
    last_frame_sample_second: Option<u64>,
}

struct AuditReportingGrantApprovalApp {
    theme_preference: ThemePreference,
    read_limit: u8,
    ttl_minutes: u64,
    outcome: Rc<RefCell<Option<AuditReportingGrantPromptDecision>>>,
    first_frame_logged: bool,
    opened_at: Instant,
    last_frame_sample_second: Option<u64>,
}

struct PasswordRotationApprovalApp {
    theme_preference: ThemePreference,
    new_password: String,
    confirm_password: String,
    error: Option<String>,
    outcome: Rc<RefCell<Option<PasswordRotationPromptDecision>>>,
    first_frame_logged: bool,
    opened_at: Instant,
    last_frame_sample_second: Option<u64>,
}

impl TokenGrantApprovalApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        theme_preference: ThemePreference,
        account: AccountPublic,
        ttl_seconds: u64,
        outcome: Rc<RefCell<Option<TokenGrantPromptDecision>>>,
    ) -> Self {
        theme::apply(&cc.egui_ctx, theme_preference);

        Self {
            theme_preference,
            account,
            ttl_seconds,
            outcome,
            first_frame_logged: false,
            opened_at: Instant::now(),
            last_frame_sample_second: None,
        }
    }

    fn approve(&mut self, ctx: &egui::Context) {
        ui_trace(
            "token_grant.approve_clicked",
            format!("account_id={}", self.account.id),
        );
        *self.outcome.borrow_mut() = Some(TokenGrantPromptDecision::Approved);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn deny(&mut self, ctx: &egui::Context) {
        ui_trace(
            "token_grant.deny_clicked",
            format!("account_id={}", self.account.id),
        );
        *self.outcome.borrow_mut() = Some(TokenGrantPromptDecision::Denied);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl ProvisioningGrantApprovalApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        theme_preference: ThemePreference,
        account_limit: u8,
        ttl_minutes: u64,
        outcome: Rc<RefCell<Option<ProvisioningGrantPromptDecision>>>,
    ) -> Self {
        theme::apply(&cc.egui_ctx, theme_preference);

        Self {
            theme_preference,
            account_limit,
            ttl_minutes,
            outcome,
            first_frame_logged: false,
            opened_at: Instant::now(),
            last_frame_sample_second: None,
        }
    }

    fn approve(&mut self, ctx: &egui::Context) {
        ui_trace(
            "provisioning_grant.approve_clicked",
            format!(
                "account_limit={} ttl_minutes={}",
                self.account_limit, self.ttl_minutes
            ),
        );
        *self.outcome.borrow_mut() = Some(ProvisioningGrantPromptDecision::Approved);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn deny(&mut self, ctx: &egui::Context) {
        ui_trace(
            "provisioning_grant.deny_clicked",
            format!(
                "account_limit={} ttl_minutes={}",
                self.account_limit, self.ttl_minutes
            ),
        );
        *self.outcome.borrow_mut() = Some(ProvisioningGrantPromptDecision::Denied);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl AuditReportingGrantApprovalApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        theme_preference: ThemePreference,
        read_limit: u8,
        ttl_minutes: u64,
        outcome: Rc<RefCell<Option<AuditReportingGrantPromptDecision>>>,
    ) -> Self {
        theme::apply(&cc.egui_ctx, theme_preference);

        Self {
            theme_preference,
            read_limit,
            ttl_minutes,
            outcome,
            first_frame_logged: false,
            opened_at: Instant::now(),
            last_frame_sample_second: None,
        }
    }

    fn approve(&mut self, ctx: &egui::Context) {
        ui_trace(
            "audit_reporting_grant.approve_clicked",
            format!(
                "read_limit={} ttl_minutes={}",
                self.read_limit, self.ttl_minutes
            ),
        );
        *self.outcome.borrow_mut() = Some(AuditReportingGrantPromptDecision::Approved);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn deny(&mut self, ctx: &egui::Context) {
        ui_trace(
            "audit_reporting_grant.deny_clicked",
            format!(
                "read_limit={} ttl_minutes={}",
                self.read_limit, self.ttl_minutes
            ),
        );
        *self.outcome.borrow_mut() = Some(AuditReportingGrantPromptDecision::Denied);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl PasswordRotationApprovalApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        theme_preference: ThemePreference,
        outcome: Rc<RefCell<Option<PasswordRotationPromptDecision>>>,
    ) -> Self {
        theme::apply(&cc.egui_ctx, theme_preference);

        Self {
            theme_preference,
            new_password: String::new(),
            confirm_password: String::new(),
            error: None,
            outcome,
            first_frame_logged: false,
            opened_at: Instant::now(),
            last_frame_sample_second: None,
        }
    }

    fn approve(&mut self, ctx: &egui::Context) {
        if self.new_password.trim().is_empty() {
            self.error = Some("La nueva contraseña maestra no puede estar vacía.".to_owned());
            self.confirm_password.zeroize();
            return;
        }

        if self.new_password != self.confirm_password {
            self.error = Some("La confirmación de la nueva contraseña no coincide.".to_owned());
            self.new_password.zeroize();
            self.confirm_password.zeroize();
            return;
        }

        self.error = None;
        self.confirm_password.zeroize();
        let new_password = SecretString::from(std::mem::take(&mut self.new_password));
        *self.outcome.borrow_mut() =
            Some(PasswordRotationPromptDecision::Approved { new_password });
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn deny(&mut self, ctx: &egui::Context) {
        self.new_password.zeroize();
        self.confirm_password.zeroize();
        *self.outcome.borrow_mut() = Some(PasswordRotationPromptDecision::Denied);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl Drop for PasswordRotationApprovalApp {
    fn drop(&mut self) {
        self.new_password.zeroize();
        self.confirm_password.zeroize();
    }
}

impl eframe::App for TokenGrantApprovalApp {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.theme_preference);
        if !self.first_frame_logged {
            ui_trace(
                "token_grant.first_frame",
                format!(
                    "ttl_seconds={} theme={:?}",
                    self.ttl_seconds, self.theme_preference
                ),
            );
            self.first_frame_logged = true;
        }
        trace_frame_sample(
            "token_grant.frame",
            ctx,
            self.opened_at,
            &mut self.last_frame_sample_second,
        );
        request_transient_repaint(ctx, self.opened_at);

        egui::TopBottomPanel::bottom("token_grant_actions")
            .resizable(false)
            .exact_height(52.0)
            .show(ctx, |ui| {
                let available = ui.available_size();
                ui.allocate_ui_with_layout(available, Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Aprobar una vez").clicked() {
                        self.approve(ctx);
                    }

                    if ui.button("Denegar").clicked() {
                        self.deny(ctx);
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.add_space(4.0);
                    ui.heading("MFA-Forge Token Grant");
                    ui.add_space(8.0);
                    ui.label(
                        "Esta aprobación permite entregar un solo TOTP por MCP. El secreto no se expone y el grant vence rápido si no se usa.",
                    );
                    ui.add_space(10.0);
                    ui.label(RichText::new("Cuenta solicitada").strong());
                    ui.monospace(format!("Servicio: {}", self.account.service));
                    ui.monospace(format!("Usuario: {}", self.account.user));
                    ui.monospace(format!("Cuenta: {}", self.account.id));
                    ui.add_space(6.0);
                    ui.label(format!(
                        "Si apruebas, este proceso podrá generar un solo token para esa cuenta durante los próximos {} segundos.",
                        self.ttl_seconds
                    ));
                });
            });
        });
    }
}

impl eframe::App for ProvisioningGrantApprovalApp {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.theme_preference);
        if !self.first_frame_logged {
            ui_trace(
                "provisioning_grant.first_frame",
                format!(
                    "account_limit={} ttl_minutes={} theme={:?}",
                    self.account_limit, self.ttl_minutes, self.theme_preference
                ),
            );
            self.first_frame_logged = true;
        }
        trace_frame_sample(
            "provisioning_grant.frame",
            ctx,
            self.opened_at,
            &mut self.last_frame_sample_second,
        );
        request_transient_repaint(ctx, self.opened_at);

        egui::TopBottomPanel::bottom("provisioning_grant_actions")
            .resizable(false)
            .exact_height(52.0)
            .show(ctx, |ui| {
                let available = ui.available_size();
                ui.allocate_ui_with_layout(available, Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Aprobar grant").clicked() {
                        self.approve(ctx);
                    }

                    if ui.button("Denegar").clicked() {
                        self.deny(ctx);
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.add_space(4.0);
                    ui.heading("MFA-Forge Provisioning Grant");
                    ui.add_space(8.0);
                    ui.label(
                        "Esta aprobación permite aprovisionar nuevas cuentas MFA por MCP sin intervención adicional hasta agotar una cuota corta. Los secretos se aceptan como input, pero no se devuelven ni se registran en el audit log.",
                    );
                    ui.add_space(10.0);
                    ui.label(RichText::new("Alcance del grant").strong());
                    ui.monospace("Tools permitidas: create_account, import_otpauth, update_account, remove_account");
                    ui.add_space(6.0);
                    ui.label(format!(
                        "Si apruebas, este proceso MCP podrá crear, importar, actualizar o eliminar hasta {} cuentas durante los próximos {} minutos.",
                        self.account_limit, self.ttl_minutes
                    ));
                });
            });
        });
    }
}

impl eframe::App for AuditReportingGrantApprovalApp {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.theme_preference);
        if !self.first_frame_logged {
            ui_trace(
                "audit_reporting_grant.first_frame",
                format!(
                    "read_limit={} ttl_minutes={} theme={:?}",
                    self.read_limit, self.ttl_minutes, self.theme_preference
                ),
            );
            self.first_frame_logged = true;
        }
        trace_frame_sample(
            "audit_reporting_grant.frame",
            ctx,
            self.opened_at,
            &mut self.last_frame_sample_second,
        );
        request_transient_repaint(ctx, self.opened_at);

        egui::TopBottomPanel::bottom("audit_reporting_grant_actions")
            .resizable(false)
            .exact_height(52.0)
            .show(ctx, |ui| {
                let available = ui.available_size();
                ui.allocate_ui_with_layout(available, Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Aprobar grant").clicked() {
                        self.approve(ctx);
                    }

                    if ui.button("Denegar").clicked() {
                        self.deny(ctx);
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.add_space(4.0);
                        ui.heading("MFA-Forge Audit Reporting Grant");
                        ui.add_space(8.0);
                        ui.label(
                            "Esta aprobación permite revisar historial público del vault y el audit log local reciente sin abrir nuevas superficies. El grant vence rápido y usa una cuota corta de lecturas.",
                        );
                        ui.add_space(10.0);
                        ui.label(RichText::new("Alcance del grant").strong());
                        ui.monospace("Tools permitidas: list_history, read_audit_events, summarize_audit_events");
                        ui.add_space(6.0);
                        ui.label(format!(
                            "Si apruebas, este proceso MCP podrá ejecutar hasta {} lecturas sensibles durante los próximos {} minutos.",
                            self.read_limit, self.ttl_minutes
                        ));
                    });
                });
        });
    }
}

impl eframe::App for PasswordRotationApprovalApp {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.theme_preference);
        if !self.first_frame_logged {
            ui_trace("password_rotation.first_frame", "window=shown");
            self.first_frame_logged = true;
        }
        trace_frame_sample(
            "password_rotation.frame",
            ctx,
            self.opened_at,
            &mut self.last_frame_sample_second,
        );
        request_transient_repaint(ctx, self.opened_at);

        egui::TopBottomPanel::bottom("password_rotation_actions")
            .resizable(false)
            .exact_height(52.0)
            .show(ctx, |ui| {
                let available = ui.available_size();
                ui.allocate_ui_with_layout(available, Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Ejecutar rotación real").clicked() {
                        self.approve(ctx);
                    }

                    if ui.button("Denegar").clicked() {
                        self.deny(ctx);
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.add_space(4.0);
                        ui.heading("MFA-Forge Password Rotation");
                        ui.add_space(8.0);
                        ui.label(
                            "Esta aprobación ejecuta una rotación real de la contraseña maestra y re-cifra el vault actual de inmediato. La nueva contraseña se captura solo en esta ventana nativa y no viaja por stdio ni por MCP.",
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(
                                "Si apruebas, la contraseña anterior deja de servir para este vault. No es una simulación ni una prueba.",
                            )
                            .strong()
                            .color(Color32::from_rgb(255, 214, 102)),
                        );
                        ui.add_space(10.0);
                        ui.label(RichText::new("Nueva contraseña maestra").strong());
                        let new_password = ui.add(
                            TextEdit::singleline(&mut self.new_password)
                                .password(true)
                                .hint_text("Nueva contraseña"),
                        );
                        let confirm_password = ui.add(
                            TextEdit::singleline(&mut self.confirm_password)
                                .password(true)
                                .hint_text("Confirmar nueva contraseña"),
                        );

                        if (new_password.lost_focus() || confirm_password.lost_focus())
                            && ui.input(|input| input.key_pressed(egui::Key::Enter))
                        {
                            self.approve(ctx);
                        }

                        if let Some(error) = &self.error {
                            ui.add_space(6.0);
                            ui.colored_label(Color32::from_rgb(229, 90, 90), error);
                        } else {
                            ui.add_space(6.0);
                            ui.label(
                                "La sesión ya desbloqueada se reutiliza para re-cifrar el vault de forma local. Si deniegas, no se aplica ningún cambio.",
                            );
                        }
                    });
                });
        });
    }
}

struct AgentUnlockApp {
    owner_window: platform_auth::OwnerWindow,
    vault: Option<VaultFacade>,
    theme_preference: ThemePreference,
    password_input: String,
    error: Option<String>,
    pending_prepare: Option<AgentPendingPrepareUnlock>,
    pending_unlock: Option<AgentPendingUnlockFlow>,
    outcome: Rc<RefCell<Option<Result<VaultFacade, String>>>>,
    first_frame_logged: bool,
    opened_at: Instant,
    last_frame_sample_second: Option<u64>,
}

impl AgentUnlockApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        theme_preference: ThemePreference,
        outcome: Rc<RefCell<Option<Result<VaultFacade, String>>>>,
    ) -> Result<Self, String> {
        theme::apply(&cc.egui_ctx, theme_preference);

        Ok(Self {
            owner_window: platform_auth::capture_owner_window(cc)?,
            vault: Some(VaultFacade::try_new().map_err(|error| error.to_string())?),
            theme_preference,
            password_input: String::new(),
            error: None,
            pending_prepare: None,
            pending_unlock: None,
            outcome,
            first_frame_logged: false,
            opened_at: Instant::now(),
            last_frame_sample_second: None,
        })
    }

    fn submit_unlock(&mut self, ctx: &egui::Context) {
        if self.pending_prepare.is_some() || self.pending_unlock.is_some() {
            ui_trace("agent_session.submit_unlock_ignored", "busy=true");
            return;
        }

        let mut password = std::mem::take(&mut self.password_input);
        if password.trim().is_empty() {
            ui_trace(
                "agent_session.submit_unlock_empty_password",
                "rejected=true",
            );
            self.error =
                Some("Ingresa la contraseña maestra para abrir la sesión del agente.".to_owned());
            password.zeroize();
            return;
        }

        let password = SecretString::from(std::mem::take(&mut password));
        if self.vault.is_none() {
            ui_trace("agent_session.submit_unlock_missing_vault", "rejected=true");
            self.error = Some("La sesión de agente no pudo inicializar el vault.".to_owned());
            return;
        }

        ui_trace(
            "agent_session.submit_unlock_started",
            "prepare_unlock=spawned",
        );
        self.error = None;
        self.pending_prepare = Some(spawn_prepare_unlock(
            password,
            |password| match VaultFacade::try_new().map_err(|error| error.to_string()) {
                Ok(vault) => vault.prepare_unlock(password),
                Err(error) => Err(error),
            },
            "La preparación del unlock",
        ));
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn cancel(&mut self, ctx: &egui::Context) {
        ui_trace("agent_session.cancel_clicked", "status=cancelled");
        self.password_input.zeroize();
        self.password_input.clear();
        *self.outcome.borrow_mut() = Some(Err(
            "El usuario canceló la apertura de la sesión de agente.".to_owned(),
        ));
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn poll_pending_prepare(&mut self, ctx: &egui::Context) {
        let poll = self
            .pending_prepare
            .as_ref()
            .map(PendingPrepareUnlock::poll);

        match poll {
            Some(PrepareUnlockPoll::Pending) => {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Some(PrepareUnlockPoll::Finished(Ok(prepared))) => {
                ui_trace(
                    "agent_session.prepare_unlock_finished",
                    "status=ok begin_verify_unlock=true",
                );
                self.pending_prepare = None;
                ui_trace(
                    "agent_session.begin_verify_unlock_start",
                    "platform=windows",
                );
                match begin_unlock_verification(prepared, &self.owner_window) {
                    Ok(pending_unlock) => {
                        ui_trace(
                            "agent_session.begin_verify_unlock_ok",
                            "pending_unlock=true",
                        );
                        self.error = None;
                        self.pending_unlock = Some(pending_unlock);
                        ctx.request_repaint_after(Duration::from_millis(100));
                    }
                    Err(error) => {
                        ui_trace(
                            "agent_session.begin_verify_unlock_error",
                            format!("error={error}"),
                        );
                        self.error = Some(error);
                    }
                }
            }
            Some(PrepareUnlockPoll::Finished(Err(error))) => {
                ui_trace(
                    "agent_session.prepare_unlock_error",
                    format!("error={error}"),
                );
                self.pending_prepare = None;
                self.error = Some(error);
            }
            None => {}
        }
    }

    fn poll_pending_unlock(&mut self, ctx: &egui::Context) {
        let poll = self.pending_unlock.as_ref().map(PendingUnlockFlow::poll);

        match poll {
            Some(VerificationPollResult::Pending) => {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Some(VerificationPollResult::Finished(Ok(()))) => {
                ui_trace("agent_session.verify_unlock_finished", "status=ok");
                if let Some(pending) = self.pending_unlock.take()
                    && let Some(mut vault) = self.vault.take()
                {
                    let (password, session) = pending.into_parts();
                    vault.finish_unlock(password, session);
                    *self.outcome.borrow_mut() = Some(Ok(vault));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Some(VerificationPollResult::Finished(Err(error))) => {
                ui_trace(
                    "agent_session.verify_unlock_error",
                    format!("error={error}"),
                );
                self.pending_unlock = None;
                self.error = Some(error);
            }
            None => {}
        }
    }
}

impl eframe::App for AgentUnlockApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.theme_preference);
        if !self.first_frame_logged {
            ui_trace("agent_session.first_frame", "window=shown");
            self.first_frame_logged = true;
        }
        trace_frame_sample(
            "agent_session.frame",
            ctx,
            self.opened_at,
            &mut self.last_frame_sample_second,
        );
        request_transient_repaint(ctx, self.opened_at);
        self.poll_pending_prepare(ctx);
        self.poll_pending_unlock(ctx);
        let is_busy = self.pending_prepare.is_some() || self.pending_unlock.is_some();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.add_space(4.0);
                ui.heading("MFA-Forge Agent Session");
                ui.add_space(8.0);
                ui.label(
                    "Esta ventana otorga una sesión temporal para un agente local. La sesión queda abierta solo mientras el proceso siga vivo.",
                );
                ui.label(
                    RichText::new(
                        "La validación adicional de Windows sigue implementada, pero continúa en revisión hasta confirmar estabilidad.",
                    )
                    .color(Color32::from_rgb(181, 208, 255)),
                );
                ui.add_space(12.0);

                let password_input = ui.add_enabled(
                    !is_busy,
                    TextEdit::singleline(&mut self.password_input)
                        .password(true)
                        .hint_text("Contraseña maestra"),
                );

                if password_input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    self.submit_unlock(ctx);
                }

                if let Some(error) = &self.error {
                    ui.add_space(6.0);
                    ui.colored_label(Color32::from_rgb(229, 90, 90), error);
                } else if self.pending_prepare.is_some() {
                    ui.add_space(6.0);
                    ui.colored_label(
                        Color32::from_rgb(181, 208, 255),
                        "Validando la contraseña contra el vault y preparando la verificación adicional.",
                    );
                } else if self.pending_unlock.is_some() {
                    ui.add_space(6.0);
                    ui.colored_label(
                        Color32::from_rgb(181, 208, 255),
                        "Contraseña correcta. Esperando la validación adicional del sistema operativo.",
                    );
                }

                ui.add_space(16.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(!is_busy, egui::Button::new("Desbloquear"))
                        .clicked()
                    {
                        self.submit_unlock(ctx);
                    }

                    if ui.button("✖ Cancelar").clicked() {
                        self.cancel(ctx);
                    }
                });
            });
        });
    }
}
