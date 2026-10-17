//! Theme previews are reversible; only an acknowledged store commit changes durable settings.
use crate::{
    domain::error::StoreError,
    store::settings::AppSettings,
    theme::{
        self, ResolvedTheme, SystemAppearance, Theme, ThemeError, ThemeId,
        contrast::{ContrastReport, check_contrast},
        palette::ThemeEntry,
    },
};

/// An immutable settings candidate tagged to the preview that requested it.
/// The coordinator sends settings() to the store and returns this ticket with its result.
pub struct AppearanceCommit {
    settings: AppSettings,
    preview_id: uuid::Uuid,
}
impl AppearanceCommit {
    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }
}

pub struct AppearanceController {
    committed: AppSettings,
    displayed: AppSettings,
    system: SystemAppearance,
    preview_id: uuid::Uuid,
}
impl AppearanceController {
    pub fn new(settings: AppSettings, system: SystemAppearance) -> Result<Self, StoreError> {
        settings.validate()?;
        Ok(Self {
            displayed: settings.clone(),
            committed: settings,
            system,
            preview_id: uuid::Uuid::new_v4(),
        })
    }
    pub fn committed(&self) -> &AppSettings {
        &self.committed
    }
    pub fn resolved(&self) -> Result<ResolvedTheme, ThemeError> {
        let themes = self
            .displayed
            .themes()
            .map_err(|_| ThemeError::InvalidSettings)?;
        theme::resolve_theme(&self.displayed.appearance, self.system, &themes)
    }
    pub fn set_system_appearance(&mut self, system: SystemAppearance) {
        self.system = system;
    }
    pub fn preview(&mut self, id: ThemeId) -> Result<ContrastReport, ThemeError> {
        let themes = self
            .displayed
            .themes()
            .map_err(|_| ThemeError::InvalidSettings)?;
        let entry = themes
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(ThemeError::UnknownTheme)?;
        let report = check_contrast(&entry.theme);
        // A warning must not replace the current palette before explicit approval.
        if report.passes() {
            self.displayed.appearance =
                theme::select_theme(&self.displayed.appearance, self.system, entry, false)?;
            self.preview_id = uuid::Uuid::new_v4();
        }
        Ok(report)
    }
    pub fn selection_candidate(
        &self,
        id: ThemeId,
        allow_low_contrast: bool,
    ) -> Result<AppSettings, ThemeError> {
        let mut candidate = self.displayed.clone();
        let themes = candidate
            .themes()
            .map_err(|_| ThemeError::InvalidSettings)?;
        let entry = themes
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(ThemeError::UnknownTheme)?;
        candidate.appearance = theme::select_theme(
            &candidate.appearance,
            self.system,
            entry,
            allow_low_contrast,
        )?;
        Ok(candidate)
    }
    /// Produces a separate candidate; neither persisted settings nor preview changes here.
    pub fn confirm_import(
        &mut self,
        theme: Theme,
        allow_low_contrast: bool,
    ) -> Result<AppSettings, ThemeError> {
        theme.validate()?;
        let entry = ThemeEntry::import(theme);
        let mut candidate = self.displayed.clone();
        candidate.appearance = theme::select_theme(
            &candidate.appearance,
            self.system,
            &entry,
            allow_low_contrast,
        )?;
        candidate.imported_themes.push(entry);
        candidate
            .validate()
            .map_err(|_| ThemeError::InvalidSettings)?;
        Ok(candidate)
    }
    pub fn preview_candidate(
        &mut self,
        candidate: &AppSettings,
        allow_low_contrast: bool,
    ) -> Result<ContrastReport, ThemeError> {
        candidate
            .validate()
            .map_err(|_| ThemeError::InvalidSettings)?;
        let resolved = theme::resolve_theme(
            &candidate.appearance,
            self.system,
            &candidate
                .themes()
                .map_err(|_| ThemeError::InvalidSettings)?,
        )?;
        let report = check_contrast(&resolved.palette);
        if !report.passes() && !allow_low_contrast {
            return Err(ThemeError::LowContrast);
        }
        self.displayed = candidate.clone();
        self.preview_id = uuid::Uuid::new_v4();
        Ok(report)
    }
    pub fn begin_settings_commit(
        &mut self,
        candidate: AppSettings,
        allow_low_contrast: bool,
    ) -> Result<AppearanceCommit, ThemeError> {
        self.preview_candidate(&candidate, allow_low_contrast)?;
        Ok(AppearanceCommit {
            settings: candidate,
            preview_id: self.preview_id,
        })
    }
    /// Complete serialized settings writes in store order. A newer preview is retained
    /// when an older request finishes, including when its settings happen to match.
    pub fn complete_settings_commit(
        &mut self,
        ticket: AppearanceCommit,
        result: Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        let still_current = ticket.preview_id == self.preview_id;
        match result {
            Ok(()) => {
                self.committed = ticket.settings;
                if still_current {
                    self.displayed = self.committed.clone();
                }
                Ok(())
            }
            Err(error) => {
                if still_current {
                    self.cancel_preview();
                }
                Err(error)
            }
        }
    }
    pub fn cancel_preview(&mut self) {
        self.displayed = self.committed.clone();
        self.preview_id = uuid::Uuid::new_v4();
    }
}
