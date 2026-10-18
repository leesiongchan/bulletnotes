//! Explicit local export only; file work does not block Slint's UI thread.
use crate::AppWindow;
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};
pub fn bind(window: &AppWindow) {
    let busy = Rc::new(Cell::new(false));
    let weak = window.as_weak();
    window.on_export_diagnostics(move || {
        if busy.replace(true) {
            return;
        }
        let pending = busy.clone();
        let weak = weak.clone();
        if slint::spawn_local(async move {
            let file = rfd::AsyncFileDialog::new()
                .set_title("Export local diagnostics")
                .set_file_name("Bulletnotes Diagnostics.jsonl")
                .add_filter("Diagnostic records", &["jsonl"])
                .save_file()
                .await;
            if let Some(file) = file {
                let path = file.path().to_owned();
                let (tx, rx) = tokio::sync::oneshot::channel();
                let started = std::thread::Builder::new()
                    .name("bulletnotes-diagnostic-export".into())
                    .spawn(move || {
                        let _ = tx.send(crate::diagnostics::export(&path));
                    });
                let success = started.is_ok() && matches!(rx.await, Ok(Ok(())));
                if let Some(window) = weak.upgrade() {
                    window.set_native_message(
                        if success {
                            "Diagnostics exported."
                        } else {
                            "Diagnostics could not be exported. Choose another location and retry."
                        }
                        .into(),
                    );
                    window.set_native_warning(!success);
                }
            }
            pending.set(false);
        })
        .is_err()
        {
            busy.set(false);
        }
    });
}
