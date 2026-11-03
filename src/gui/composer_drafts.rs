//! Account/thread-scoped composer persistence, with late-load and close acknowledgements.

use super::*;

#[derive(Default)]
pub(super) struct ComposerDraft {
    owner: String,
    thread: String,
    generation: u64,
    observed: String,
    persisted: String,
    queued: Option<String>,
    touched: bool,
    changed: Option<std::time::Instant>,
    close_pending: bool,
}

impl ZaloGuiApp {
    pub(super) fn sync_composer_draft(&mut self, ctx: &egui::Context) {
        if self.smoke.is_some() {
            return;
        }
        let owner = if self.is_logged_in {
            self.user_uin.clone()
        } else {
            String::new()
        };
        let thread = if owner.is_empty() {
            String::new()
        } else {
            self.threads
                .get(self.selected_thread_index)
                .map(|t| t.id.clone())
                .unwrap_or_default()
        };
        if self.composer_draft.owner != owner || self.composer_draft.thread != thread {
            // Account-change handlers may already have cleared the editor. Persist its last
            // observed value under the old account rather than clearing that account's draft.
            if self.composer_draft.owner == owner {
                self.observe_composer_draft();
            }
            self.flush_composer_draft();
            let generation = self.composer_draft.generation.wrapping_add(1);
            self.composer_draft = ComposerDraft {
                owner: owner.clone(),
                thread: thread.clone(),
                generation,
                ..Default::default()
            };
            self.input_message.clear();
            if !owner.is_empty() && !thread.is_empty() {
                if let Some(tx) = WORKER_TX.get() {
                    if tx
                        .send(WorkerCmd::LoadComposerDraft {
                            owner,
                            thread,
                            generation,
                        })
                        .is_err()
                    {
                        self.status_msg = "Không đọc được bản nháp: bộ xử lý đã dừng".into();
                    }
                }
            }
        } else {
            self.observe_composer_draft();
        }
        let draft = &self.composer_draft;
        if draft.touched
            && (draft.observed != draft.persisted || draft.queued.is_some())
            && draft.queued.as_deref() != Some(&draft.observed)
        {
            let elapsed = draft.changed.map(|at| at.elapsed()).unwrap_or_default();
            if elapsed >= std::time::Duration::from_millis(500) {
                self.flush_composer_draft();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(500) - elapsed);
            }
        }
    }

    pub(super) fn discard_deleted_conversation_draft(&mut self, owner: &str, thread: &str) {
        let draft = &mut self.composer_draft;
        if draft.owner == owner && draft.thread == thread {
            self.input_message.clear();
            draft.observed.clear();
            draft.persisted.clear();
            draft.queued = None;
            draft.touched = false;
            if let Some(tx) = WORKER_TX.get() {
                if tx
                    .send(WorkerCmd::SaveComposerDraft {
                        owner: owner.into(),
                        thread: thread.into(),
                        generation: draft.generation,
                        text: String::new(),
                    })
                    .is_err()
                {
                    self.status_msg = "Không xóa được bản nháp của hội thoại".into();
                }
            }
        }
    }

    pub(super) fn flush_composer_before_logout(&mut self) {
        self.observe_composer_draft();
        self.flush_composer_draft();
    }

    fn observe_composer_draft(&mut self) {
        let draft = &mut self.composer_draft;
        if !draft.thread.is_empty() && self.input_message != draft.observed {
            draft.observed = self.input_message.clone();
            draft.touched = true;
            draft.changed = Some(std::time::Instant::now());
        }
    }

    fn flush_composer_draft(&mut self) {
        let draft = &mut self.composer_draft;
        if !draft.touched
            || draft.owner.is_empty()
            || draft.thread.is_empty()
            || (draft.observed == draft.persisted && draft.queued.is_none())
            || draft.queued.as_deref() == Some(&draft.observed)
        {
            return;
        }
        if draft.observed.len() > 64 * 1024 {
            self.status_msg = "Bản nháp vượt 64 KiB; hãy rút ngắn nội dung để lưu".into();
            return;
        }
        if let Some(tx) = WORKER_TX.get() {
            let text = draft.observed.clone();
            if tx
                .send(WorkerCmd::SaveComposerDraft {
                    owner: draft.owner.clone(),
                    thread: draft.thread.clone(),
                    generation: draft.generation,
                    text: text.clone(),
                })
                .is_ok()
            {
                draft.queued = Some(text);
            } else {
                self.status_msg = "Không lưu được bản nháp: bộ xử lý đã dừng".into();
            }
        }
    }

    pub(super) fn composer_draft_loaded(
        &mut self,
        owner: String,
        thread: String,
        generation: u64,
        result: Result<String, String>,
    ) {
        let draft = &mut self.composer_draft;
        if draft.owner != owner || draft.thread != thread || draft.generation != generation {
            return;
        }
        match result {
            Ok(text) => {
                // Keystrokes, send, and explicit clearing all win over an asynchronous load.
                if !draft.touched && self.input_message == draft.observed {
                    self.input_message = text.clone();
                    draft.observed = text.clone();
                    draft.persisted = text;
                }
            }
            Err(error) => self.status_msg = error,
        }
    }

    pub(super) fn composer_draft_saved(
        &mut self,
        ctx: &egui::Context,
        owner: String,
        thread: String,
        generation: u64,
        text: String,
        result: Result<(), String>,
    ) {
        let draft = &mut self.composer_draft;
        if draft.owner != owner || draft.thread != thread || draft.generation != generation {
            return;
        }
        if draft.queued.as_ref() == Some(&text) {
            draft.queued = None;
        }
        match result {
            Ok(()) => {
                draft.persisted = text;
                if draft.close_pending
                    && draft.queued.is_none()
                    && self.input_message == draft.persisted
                {
                    draft.close_pending = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Err(error) => {
                draft.close_pending = false;
                draft.changed = Some(std::time::Instant::now());
                self.status_msg = error;
            }
        }
    }

    pub(super) fn preserve_draft_before_close(&mut self, ctx: &egui::Context) {
        if self.smoke.is_some()
            || (!self.allow_quit && !ctx.input(|i| i.viewport().close_requested()))
        {
            return;
        }
        self.observe_composer_draft();
        if self.composer_draft.touched
            && (self.composer_draft.observed != self.composer_draft.persisted
                || self.composer_draft.queued.is_some())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.composer_draft.close_pending = true;
            self.status_msg = "Đang lưu bản nháp trước khi đóng…".into();
            self.flush_composer_draft();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app(ctx: &egui::Context) -> ZaloGuiApp {
        let session = Session::new(false, false, false).unwrap();
        let mut app = ZaloGuiApp::from_context(
            ctx,
            session,
            false,
            vec![
                Thread {
                    id: "thread-a".into(),
                    ..Default::default()
                },
                Thread {
                    id: "thread-b".into(),
                    ..Default::default()
                },
            ],
            false,
        );
        app.selected_thread_index = 0;
        app.is_logged_in = true;
        app.user_uin = "owner-a".into();
        app.sync_composer_draft(ctx);
        app
    }
    #[test]
    fn late_load_never_overwrites_typing_or_an_explicit_clear() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let generation = app.composer_draft.generation;
        app.input_message = "Tiếng Việt vừa gõ".into();
        app.sync_composer_draft(&ctx);
        app.composer_draft_loaded(
            "owner-a".into(),
            "thread-a".into(),
            generation,
            Ok("cũ".into()),
        );
        assert_eq!(app.input_message, "Tiếng Việt vừa gõ");
        app.input_message.clear();
        app.sync_composer_draft(&ctx);
        app.composer_draft_loaded(
            "owner-a".into(),
            "thread-a".into(),
            generation,
            Ok("cũ".into()),
        );
        assert!(app.input_message.is_empty());
    }
    #[test]
    fn old_thread_and_old_account_loads_do_not_cross_contexts() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let generation = app.composer_draft.generation;
        app.selected_thread_index = 1;
        app.sync_composer_draft(&ctx);
        app.composer_draft_loaded(
            "owner-a".into(),
            "thread-a".into(),
            generation,
            Ok("nháp A".into()),
        );
        assert!(app.input_message.is_empty());
        let generation = app.composer_draft.generation;
        app.composer_draft_loaded(
            "owner-a".into(),
            "thread-b".into(),
            generation,
            Ok("  nháp B\n".into()),
        );
        assert_eq!(app.input_message, "  nháp B\n");
        app.user_uin = "owner-b".into();
        app.sync_composer_draft(&ctx);
        app.composer_draft_loaded(
            "owner-a".into(),
            "thread-b".into(),
            generation,
            Ok("khác tài khoản".into()),
        );
        assert!(app.input_message.is_empty());
    }
    #[test]
    fn old_save_ack_does_not_clear_a_newer_empty_save() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let generation = app.composer_draft.generation;
        app.composer_draft.touched = true;
        app.composer_draft.queued = Some(String::new());
        app.composer_draft_saved(
            &ctx,
            "owner-a".into(),
            "thread-a".into(),
            generation,
            "tin đã gửi".into(),
            Ok(()),
        );
        assert_eq!(app.composer_draft.queued, Some(String::new()));
        assert_eq!(app.composer_draft.persisted, "tin đã gửi");
        app.composer_draft_saved(
            &ctx,
            "owner-a".into(),
            "thread-a".into(),
            generation,
            String::new(),
            Ok(()),
        );
        assert!(app.composer_draft.queued.is_none());
        assert!(app.composer_draft.persisted.is_empty());
    }
}

#[cfg(test)]
mod close_tests {
    use super::*;
    #[test]
    fn close_waits_for_the_latest_draft_acknowledgement() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.composer_draft = ComposerDraft {
            owner: "owner-a".into(),
            thread: "thread-a".into(),
            generation: 1,
            touched: true,
            queued: Some("older text".into()),
            ..Default::default()
        };
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input, |ctx| app.preserve_draft_before_close(ctx));
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::CancelClose)));
        app.composer_draft.queued = Some(String::new());
        let output = ctx.run(Default::default(), |ctx| {
            app.composer_draft_saved(
                ctx,
                "owner-a".into(),
                "thread-a".into(),
                1,
                "older text".into(),
                Ok(()),
            )
        });
        assert!(!output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::Close)));
        let output = ctx.run(Default::default(), |ctx| {
            app.composer_draft_saved(
                ctx,
                "owner-a".into(),
                "thread-a".into(),
                1,
                String::new(),
                Ok(()),
            )
        });
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::Close)));
    }
}

#[cfg(test)]
mod explicit_quit_tests {
    use super::*;
    #[test]
    fn ctrl_q_or_tray_quit_waits_even_before_a_close_event_arrives() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.allow_quit = true;
        app.composer_draft = ComposerDraft {
            owner: "owner-a".into(),
            thread: "thread-a".into(),
            generation: 1,
            touched: true,
            observed: "nháp mới".into(),
            queued: Some("nháp mới".into()),
            ..Default::default()
        };
        app.input_message = "nháp mới".into();
        let output = ctx.run(Default::default(), |ctx| {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            app.preserve_draft_before_close(ctx);
        });
        assert!(app.composer_draft.close_pending);
        assert!(matches!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .last(),
            Some(egui::ViewportCommand::CancelClose)
        ));
    }
}
