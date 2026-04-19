//! Geniuz tray — Windows ambient presence + minimal status window.
//!
//! Checkpoint 1: tray icon + right-click menu (Open / About / Quit),
//! double-click opens a tiny status window showing memory count.
//! No polling, no configure flow, no recent memories — those land in checkpoint 2+.

// Don't allocate a console window on Windows — this is a GUI app.
// The `windows` subsystem is the PE-level flag that tells Windows not to
// attach stdin/stdout/stderr to a console. Without this, Windows creates
// a black console window on launch.
#![windows_subsystem = "windows"]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, ControlFlow};
use winit::window::WindowId;

/// Shared state between the tray event loop and the status window.
#[derive(Clone, Default)]
struct Status {
    memory_count: usize,
    station_exists: bool,
}

impl Status {
    /// Read current status from the Geniuz memory.db. No embedding calls here —
    /// just counts. Errors degrade to zeroes so the UI always has something to render.
    fn fetch() -> Self {
        let station_path = geniuz_memory_db_path();
        if !station_path.exists() {
            return Status::default();
        }
        let Ok(db) = geniuz::db::DatabaseManager::new(&station_path.to_string_lossy()) else {
            return Status::default();
        };
        let memory_count = db.count().unwrap_or(0);
        Status {
            memory_count,
            station_exists: true,
        }
    }
}

fn geniuz_memory_db_path() -> PathBuf {
    // Matches the Rust CLI's path resolution: GENIUZ_HOME env var, else ~/.geniuz/memory.db
    if let Ok(home) = std::env::var("GENIUZ_HOME") {
        return PathBuf::from(home).join("memory.db");
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".geniuz").join("memory.db")
}

// =============================================================================
// Status window — egui
// =============================================================================

struct StatusApp {
    status: Arc<Mutex<Status>>,
}

impl eframe::App for StatusApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(16.0);
                ui.heading("Geniuz");
                ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
                ui.add_space(24.0);

                let status = self.status.lock().unwrap().clone();
                if status.station_exists {
                    ui.label(egui::RichText::new(format!("{} memories", status.memory_count))
                        .size(20.0));
                } else {
                    ui.label("No memories yet.");
                    ui.label(egui::RichText::new("Start a conversation in Claude Desktop.")
                        .size(12.0)
                        .color(egui::Color32::GRAY));
                }
                ui.add_space(16.0);
                ui.label(egui::RichText::new("Checkpoint 1 — scaffold verification")
                    .size(10.0)
                    .color(egui::Color32::DARK_GRAY));
            });
        });
    }
}

fn show_status_window(status: Arc<Mutex<Status>>) {
    // Refresh on open.
    *status.lock().unwrap() = Status::fetch();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([360.0, 280.0])
            .with_title("Geniuz")
            .with_resizable(false),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Geniuz",
        options,
        Box::new(|_cc| Ok(Box::new(StatusApp { status }))),
    );
}

// =============================================================================
// Tray — winit event loop owns the message pump
// =============================================================================

struct TrayApp {
    _tray: tray_icon::TrayIcon,
    open_id: tray_icon::menu::MenuId,
    about_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
    status: Arc<Mutex<Status>>,
}

impl ApplicationHandler for TrayApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _: WindowId,
        _: WindowEvent,
    ) {}

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Poll menu + tray events once per tick.
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.open_id {
                show_status_window(self.status.clone());
            } else if event.id == self.about_id {
                show_about_window();
            } else if event.id == self.quit_id {
                event_loop.exit();
            }
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::DoubleClick { .. } = event {
                show_status_window(self.status.clone());
            }
        }
        event_loop.set_control_flow(ControlFlow::wait_duration(
            std::time::Duration::from_millis(100),
        ));
    }
}

fn show_about_window() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 180.0])
            .with_title("About Geniuz")
            .with_resizable(false),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "About Geniuz",
        options,
        Box::new(|_| Ok(Box::new(AboutApp))),
    );
}

struct AboutApp;

impl eframe::App for AboutApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading("Geniuz");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.add_space(12.0);
                ui.label("Your AI remembers now.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Managed Ventures LLC")
                    .size(11.0)
                    .color(egui::Color32::GRAY));
            });
        });
    }
}

// =============================================================================
// Tray icon loading — embed a 32x32 PNG
// =============================================================================

fn load_tray_icon() -> tray_icon::Icon {
    let png = include_bytes!("../images/tray-icon.png");
    let img = image::load_from_memory(png)
        .expect("tray icon png decode")
        .to_rgba8();
    let (w, h) = img.dimensions();
    tray_icon::Icon::from_rgba(img.into_raw(), w, h).expect("tray icon from rgba")
}

// =============================================================================
// Main
// =============================================================================

fn main() {
    let status = Arc::new(Mutex::new(Status::fetch()));

    let menu = Menu::new();
    let open = MenuItem::new("Open Geniuz", true, None);
    let about = MenuItem::new("About", true, None);
    let sep = PredefinedMenuItem::separator();
    let quit = MenuItem::new("Quit Geniuz", true, None);
    menu.append(&open).unwrap();
    menu.append(&about).unwrap();
    menu.append(&sep).unwrap();
    menu.append(&quit).unwrap();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(format!("Geniuz — {} memories", status.lock().unwrap().memory_count))
        .with_icon(load_tray_icon())
        .build()
        .expect("tray icon build");

    let mut app = TrayApp {
        _tray: tray,
        open_id: open.id().clone(),
        about_id: about.id().clone(),
        quit_id: quit.id().clone(),
        status,
    };

    let event_loop = EventLoop::new().expect("winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app).expect("tray run");
}
