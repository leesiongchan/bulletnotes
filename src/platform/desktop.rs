//! Portable desktop operations through Slint's pinned Winit integration.
use crate::{AppWindow, store::settings::WindowPlacement, theme::SystemAppearance};
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};
use std::{cell::RefCell, path::PathBuf};

pub struct NativeApplication {
    window: RefCell<Option<slint::Weak<AppWindow>>>,
}
impl NativeApplication {
    pub fn prepare() -> Result<Self, &'static str> {
        Ok(Self {
            window: RefCell::new(None),
        })
    }
    pub fn bind_quit(&mut self, window: &AppWindow) {
        self.window.replace(Some(window.as_weak()));
        let weak = window.as_weak();
        window.window().on_close_requested(move || {
            if let Some(window) = weak.upgrade() {
                window.invoke_request_hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }
    pub fn activate(&self) {
        if let Some(window) = self.window.borrow().as_ref().and_then(slint::Weak::upgrade) {
            window
                .window()
                .with_winit_window(|native| native.focus_window());
        }
    }
    pub fn activate_store_owner(&self, _pid: u32) -> Result<(), &'static str> {
        // The store lock remains authoritative; never activate an unverified PID.
        Err("Bulletnotes is already running; reopen it from the taskbar or tray")
    }
    pub fn system_appearance(&self) -> SystemAppearance {
        self.window
            .borrow()
            .as_ref()
            .and_then(slint::Weak::upgrade)
            .map(|window| system_appearance(window.window()))
            .unwrap_or(SystemAppearance::Light)
    }
}

pub fn system_appearance(window: &slint::Window) -> SystemAppearance {
    match window.with_winit_window(|native| native.theme()).flatten() {
        Some(winit::window::Theme::Dark) => SystemAppearance::Dark,
        _ => SystemAppearance::Light,
    }
}

pub fn database_path() -> Result<PathBuf, &'static str> {
    let project = directories::ProjectDirs::from("lol", "harflix", "Bulletnotes")
        .ok_or("local application data directory is unavailable")?;
    let directory = project.data_local_dir();
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(directory)
        .map_err(|_| "Bulletnotes data directory is unavailable")?;
    Ok(directory.join("bulletnotes.sqlite"))
}

pub fn select() -> Result<(), slint::PlatformError> {
    let selector = slint::BackendSelector::new().backend_name("winit".into());
    if software_renderer() {
        return selector.renderer_name("software".into()).select();
    }
    let mut settings = slint::wgpu_29::WGPUSettings::default();
    settings.backends = if cfg!(target_os = "windows") {
        slint::wgpu_29::wgpu::Backends::DX12 | slint::wgpu_29::wgpu::Backends::VULKAN
    } else if cfg!(target_os = "macos") {
        slint::wgpu_29::wgpu::Backends::METAL
    } else {
        slint::wgpu_29::wgpu::Backends::VULKAN
    };
    selector
        .renderer_name("skia".into())
        .require_wgpu_29(slint::wgpu_29::WGPUConfiguration::Automatic(settings))
        .select()
}

pub fn software_renderer() -> bool {
    std::env::var_os("SLINT_BACKEND").is_some_and(|value| value == "winit-software")
}

pub fn capture(window: &AppWindow) -> Result<WindowPlacement, Box<dyn std::error::Error>> {
    let position = window.window().position();
    let size = window.window().size();
    Ok(WindowPlacement {
        x: f64::from(position.x),
        y: f64::from(position.y),
        width: f64::from(size.width),
        height: f64::from(size.height),
        pinned: window.get_is_pinned(),
    })
}

pub fn restore_window(
    window: &AppWindow,
    saved: Option<&WindowPlacement>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::app::window_placement::{self, ScreenFrame};
    let screens = window
        .window()
        .with_winit_window(|native| {
            native
                .available_monitors()
                .map(|monitor| {
                    let position = monitor.position();
                    let size = monitor.size();
                    ScreenFrame {
                        x: f64::from(position.x),
                        y: f64::from(position.y),
                        width: f64::from(size.width),
                        height: f64::from(size.height),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(saved) = saved.and_then(|saved| window_placement::restore(saved, &screens)) {
        window.window().set_size(slint::PhysicalSize::new(
            saved.width as u32,
            saved.height as u32,
        ));
        window
            .window()
            .set_position(slint::PhysicalPosition::new(saved.x as i32, saved.y as i32));
        window.set_is_pinned(saved.pinned);
    }
    Ok(())
}
