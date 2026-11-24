//! Server privacy controls remain unknown until read for the active account.
use super::*;
#[derive(Default)]
pub(super) struct PrivacyPanel {
    pub snapshot: Option<crate::privacy::Snapshot>,
    pub pending: bool,
    pub uncertain: bool,
    pub generation: u64,
    pub status: String,
    pub confirm: Option<(crate::privacy::Setting, i64)>,
}
impl ZaloGuiApp {
    fn privacy_command(&mut self, action: crate::privacy::Action) {
        if self.privacy_panel.pending {
            return;
        }
        self.privacy_panel.pending = true;
        self.privacy_panel.generation = crate::search::generation();
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Privacy {
                owner: self.user_uin.clone(),
                runtime_id: self.account_runtime_id,
                generation: self.privacy_panel.generation,
                action,
            })
            .is_ok()
        }) {
            self.privacy_panel.pending = false;
            self.privacy_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn render_server_privacy(&mut self, ui: &mut egui::Ui) {
        use crate::privacy::{Action, Setting};
        ui.heading("Quyền riêng tư trên máy chủ");
        ui.label("Đây là thiết lập tài khoản, áp dụng cả trên các thiết bị khác. Trường chưa được máy chủ cung cấp sẽ không hiện để sửa.");
        let mut intent = None;
        if ui.button("Danh sách chặn tin nhắn…").clicked() {
            self.open_blocklist();
        }
        if ui
            .add_enabled(
                !self.privacy_panel.pending,
                egui::Button::new("Tải lại quyền riêng tư"),
            )
            .clicked()
        {
            intent = Some(Action::Read);
        }
        if self.privacy_panel.pending {
            ui.spinner();
        }
        ui.label(&self.privacy_panel.status);
        if let Some(snapshot) = &self.privacy_panel.snapshot {
            ui.add_enabled_ui(
                !self.privacy_panel.pending && !self.privacy_panel.uncertain,
                |ui| {
                    for (setting, label) in [
                        (Setting::StrangerDirect, "Hiện tin người lạ trực tiếp"),
                        (Setting::ShowOnline, "Hiện trạng thái truy cập"),
                        (Setting::DisplaySeen, "Hiện trạng thái đã xem"),
                    ] {
                        if let Some(value) = snapshot.values.get(&setting) {
                            let mut enabled = *value == 1;
                            if ui.checkbox(&mut enabled, label).changed() {
                                self.privacy_panel.confirm = Some((setting, i64::from(enabled)));
                            }
                        }
                    }
                    for (setting, label, options) in [
                        (
                            Setting::ReceiveMessages,
                            "Cho phép nhắn tin",
                            vec![(1, "Mọi người"), (2, "Bạn bè")],
                        ),
                        (
                            Setting::ReceiveCalls,
                            "Cho phép cuộc gọi",
                            vec![
                                (2, "Bạn bè"),
                                (4, "Bạn bè và người lạ từng liên hệ"),
                                (3, "Mọi người"),
                            ],
                        ),
                    ] {
                        if let Some(value) = snapshot.values.get(&setting) {
                            let mut selected = *value;
                            egui::ComboBox::from_id_salt(("privacy-setting", setting.key()))
                                .selected_text(
                                    options
                                        .iter()
                                        .find(|(value, _)| *value == selected)
                                        .map(|(_, label)| *label)
                                        .unwrap_or("Chưa xác định"),
                                )
                                .show_ui(ui, |ui| {
                                    for (value, label) in &options {
                                        ui.selectable_value(&mut selected, *value, *label);
                                    }
                                });
                            ui.label(label);
                            if selected != *value {
                                self.privacy_panel.confirm = Some((setting, selected));
                            }
                        }
                    }
                },
            );
        }
        if let Some((setting, value)) = self.privacy_panel.confirm {
            ui.separator();
            ui.label("Áp dụng thay đổi quyền riêng tư này cho tài khoản trên máy chủ?");
            if ui
                .add_enabled(
                    !self.privacy_panel.pending,
                    egui::Button::new("Xác nhận thay đổi"),
                )
                .clicked()
            {
                intent = Some(Action::Set { setting, value });
                self.privacy_panel.confirm = None;
            }
            if ui.button("Hủy").clicked() {
                self.privacy_panel.confirm = None;
            }
        }
        if self.privacy_panel.uncertain {
            ui.label("Chưa rõ kết quả thay đổi. Tải lại quyền riêng tư trước khi sửa tiếp.");
        }
        if let Some(action) = intent {
            self.privacy_command(action);
        }
    }
}

#[cfg(test)]
mod remediation_privacy_tests {
    use super::*;

    fn snapshot(enabled: i64) -> crate::privacy::Snapshot {
        crate::privacy::Snapshot {
            values: [(crate::privacy::Setting::ShowOnline, enabled)]
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn privacy_command_ack_updates_gate_and_fences_older_metadata() {
        for invalidated_during_request in [true, false] {
            let ctx = egui::Context::default();
            let mut app = ZaloGuiApp::from_context(
                &ctx,
                Session::new(false, false, false).unwrap(),
                false,
                Vec::new(),
                false,
            );
            app.is_logged_in = true;
            app.user_uin = "privacy-owner".into();
            app.account_runtime_id = 42;
            app.inbox_policy.show_online = Some(true);
            app.contact_panel.presence = Some(crate::last_activity::Status::LastActivity(1));
            let old_revision = crate::search::generation();
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            WORKER_TX.set(worker::WorkerSender::new_for_test(tx, ctx.clone()));
            app.privacy_command(crate::privacy::Action::Set {
                setting: crate::privacy::Setting::ShowOnline,
                value: 0,
            });
            let WorkerCmd::Privacy {
                owner, generation, ..
            } = rx.try_recv().unwrap()
            else {
                panic!("expected privacy command");
            };
            assert!(app.privacy_panel.generation > old_revision);
            if invalidated_during_request {
                app.event_tx
                    .send(GuiEvent::AccountMetadataInvalidated {
                        runtime_id: app.account_runtime_id,
                        revision: crate::search::generation(),
                        owner: owner.clone(),
                        privacy: true,
                        relations: false,
                        business: false,
                    })
                    .unwrap();
            }
            app.event_tx
                .send(GuiEvent::Privacy {
                    runtime_id: app.account_runtime_id,
                    owner: owner.clone(),
                    generation,
                    mutation: true,
                    result: Ok(snapshot(0)),
                })
                .unwrap();
            app.event_tx
                .send(GuiEvent::AccountMetadata {
                    owner,
                    runtime_id: 42,
                    revision: old_revision,
                    snapshot: Box::new(crate::account_metadata::Snapshot {
                        privacy: Some(snapshot(1)),
                        ..Default::default()
                    }),
                })
                .unwrap();
            app.check_events(&ctx);
            assert!(!app.privacy_panel.pending);
            assert_eq!(app.privacy_panel.uncertain, invalidated_during_request);
            assert_eq!(
                app.inbox_policy.show_online,
                if invalidated_during_request {
                    None
                } else {
                    Some(false)
                }
            );
            assert!(app.contact_panel.presence.is_none());
            if invalidated_during_request {
                app.privacy_command(crate::privacy::Action::Read);
                let WorkerCmd::Privacy {
                    owner, generation, ..
                } = rx.try_recv().unwrap()
                else {
                    panic!("expected recovery read");
                };
                app.event_tx
                    .send(GuiEvent::Privacy {
                        runtime_id: app.account_runtime_id,
                        owner,
                        generation,
                        mutation: false,
                        result: Ok(snapshot(0)),
                    })
                    .unwrap();
                app.check_events(&ctx);
                assert!(!app.privacy_panel.pending);
                assert!(!app.privacy_panel.uncertain);
                assert_eq!(app.inbox_policy.show_online, Some(false));
            }
            WORKER_TX.clear();
        }
    }

    #[test]
    fn delayed_invalidation_hides_pending_read_but_accepts_its_newer_result() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "privacy-owner".into();
        app.inbox_policy.show_online = Some(true);
        let invalidation_revision = crate::search::generation();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        WORKER_TX.set(worker::WorkerSender::new_for_test(tx, ctx.clone()));
        app.privacy_command(crate::privacy::Action::Read);
        let WorkerCmd::Privacy {
            owner, generation, ..
        } = rx.try_recv().unwrap()
        else {
            panic!("expected read");
        };
        app.event_tx
            .send(GuiEvent::AccountMetadataInvalidated {
                runtime_id: app.account_runtime_id,
                owner: owner.clone(),
                revision: invalidation_revision,
                privacy: true,
                relations: false,
                business: false,
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.privacy_panel.pending);
        assert_eq!(app.inbox_policy.show_online, None);
        app.event_tx
            .send(GuiEvent::Privacy {
                runtime_id: app.account_runtime_id,
                owner,
                generation,
                mutation: false,
                result: Ok(snapshot(0)),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(!app.privacy_panel.pending);
        assert!(!app.privacy_panel.uncertain);
        assert_eq!(app.inbox_policy.show_online, Some(false));
        WORKER_TX.clear();
    }
}
