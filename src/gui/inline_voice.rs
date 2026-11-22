//! Inline voice playback. UI owns controls; worker owns download, decoder and device lifetime.
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone, Debug, Default)]
pub struct Controls {
    pub stop: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub owner: String,
    pub runtime: usize,
    pub thread: String,
    pub message: String,
    pub request: u64,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub loading: bool,
    pub paused: bool,
    pub finished: bool,
    pub position: f32,
    pub duration: f32,
    pub error: Option<String>,
}
pub(super) struct Active {
    binding: Binding,
    controls: Controls,
    snapshot: Snapshot,
}
impl Drop for Active {
    fn drop(&mut self) {
        self.controls.stop.store(true, Ordering::Release);
    }
}
pub(super) fn view_key(message: &str) -> Id {
    Id::new(("inline_voice_playback", message))
}

impl ZaloGuiApp {
    pub(super) fn voice_transcript_target_valid(&self, thread: &str, message: &str) -> bool {
        self.threads
            .iter()
            .find(|target| target.id == thread)
            .is_some_and(|target| {
                !self.content_protected(thread, target.is_group)
                    && target.messages.iter().any(|item| {
                        item.id == message
                            && item.kind == crate::message_model::MessageKind::Voice
                            && !item.is_e2ee
                            && !crate::message_lifecycle::expired(
                                item,
                                crate::login::unix_now() * 1000,
                            )
                    })
            })
    }
    pub(super) fn toggle_inline_voice(
        &mut self,
        ctx: &egui::Context,
        thread: &str,
        group: bool,
        message: ChatMessage,
    ) {
        if let Some(active) = self.inline_voice.as_mut().filter(|active| {
            active.binding.owner == self.user_uin
                && active.binding.runtime == self.account_runtime_id
                && active.binding.thread == thread
                && active.binding.message == message.id
                && !active.snapshot.finished
                && active.snapshot.error.is_none()
        }) {
            if !active.snapshot.loading {
                let paused = !active.controls.pause.load(Ordering::Acquire);
                active.controls.pause.store(paused, Ordering::Release);
            }
            return;
        }
        self.clear_inline_voice(ctx);
        let binding = Binding {
            owner: self.user_uin.clone(),
            runtime: self.account_runtime_id,
            thread: thread.to_owned(),
            message: message.id.clone(),
            request: crate::chat::next_client_message_id(),
        };
        let controls = Controls::default();
        let snapshot = Snapshot {
            loading: true,
            ..Default::default()
        };
        ctx.data_mut(|data| data.insert_temp(view_key(&message.id), snapshot.clone()));
        let command = WorkerCmd::PlayInlineVoice {
            binding: binding.clone(),
            controls: controls.clone(),
            context: worker::ChatImage {
                thread: thread.to_owned(),
                group,
                message,
                preview: false,
            },
        };
        match WORKER_TX.get().map(|tx| tx.send(command)) {
            Some(Ok(())) => {
                self.inline_voice = Some(Active {
                    binding,
                    controls,
                    snapshot,
                })
            }
            _ => {
                ctx.data_mut(|data| data.remove::<Snapshot>(view_key(&binding.message)));
                self.show_toast("Không kết nối được bộ phát âm thanh");
            }
        }
    }
    pub(super) fn clear_inline_voice(&mut self, ctx: &egui::Context) {
        if let Some(active) = self.inline_voice.take() {
            ctx.data_mut(|data| data.remove::<Snapshot>(view_key(&active.binding.message)));
        }
    }
    pub(super) fn reconcile_inline_voice(&mut self, ctx: &egui::Context) {
        let valid = self.inline_voice.as_ref().is_none_or(|active| {
            self.is_logged_in
                && active.binding.owner == self.user_uin
                && active.binding.runtime == self.account_runtime_id
                && self
                    .threads
                    .get(self.selected_thread_index)
                    .is_some_and(|thread| {
                        thread.id == active.binding.thread
                            && thread.messages.iter().any(|message| {
                                message.id == active.binding.message
                                    && message.kind != crate::message_model::MessageKind::Recall
                                    && !crate::message_lifecycle::expired(
                                        message,
                                        crate::login::unix_now() * 1000,
                                    )
                            })
                    })
        });
        if !valid {
            self.clear_inline_voice(ctx);
        }
    }
    pub(super) fn inline_voice_updated(
        &mut self,
        ctx: &egui::Context,
        binding: Binding,
        snapshot: Snapshot,
    ) {
        if let Some(active) = self.inline_voice.as_mut().filter(|active| {
            active.binding == binding
                && binding.owner == self.user_uin
                && binding.runtime == self.account_runtime_id
        }) {
            ctx.data_mut(|data| data.insert_temp(view_key(&binding.message), snapshot.clone()));
            active.snapshot = snapshot;
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_transcript_does_not_replace_recalled_voice() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "thread".into(),
                messages: vec![ChatMessage {
                    id: "voice".into(),
                    kind: crate::message_model::MessageKind::Recall,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            false,
        );
        app.user_uin = "owner".into();
        app.is_logged_in = true;
        app.transcribing_voices
            .insert(("thread".into(), "voice".into()));
        app.event_tx
            .send(GuiEvent::VoiceTranscriptLoaded {
                thread_id: "thread".into(),
                msg_id: "voice".into(),
                transcript: "must not reappear".into(),
                generation: app.account_runtime_id as u64,
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.voice_transcripts.is_empty());
        assert!(app.transcribing_voices.is_empty());
    }
    fn binding() -> Binding {
        Binding {
            owner: "owner".into(),
            runtime: 7,
            thread: "thread".into(),
            message: "voice".into(),
            request: 1,
        }
    }
    #[test]
    fn replacing_voice_releases_previous_device_owner() {
        let controls = Controls::default();
        let active = Active {
            binding: binding(),
            controls: controls.clone(),
            snapshot: Snapshot::default(),
        };
        assert!(!controls.stop.load(Ordering::Acquire));
        drop(active);
        assert!(controls.stop.load(Ordering::Acquire));
    }
    #[test]
    fn stale_voice_result_cannot_replace_current_request_or_account() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.user_uin = "owner".into();
        app.account_runtime_id = 7;
        let current = binding();
        app.inline_voice = Some(Active {
            binding: current.clone(),
            controls: Controls::default(),
            snapshot: Snapshot {
                loading: true,
                ..Default::default()
            },
        });
        for changed in 0..3 {
            let mut stale = current.clone();
            match changed {
                0 => stale.owner = "other".into(),
                1 => stale.runtime = 8,
                _ => stale.request = 2,
            }
            app.inline_voice_updated(
                &ctx,
                stale,
                Snapshot {
                    duration: 10.0,
                    ..Default::default()
                },
            );
            assert!(app.inline_voice.as_ref().unwrap().snapshot.loading);
        }
        app.inline_voice_updated(
            &ctx,
            current,
            Snapshot {
                duration: 3.0,
                ..Default::default()
            },
        );
        assert_eq!(app.inline_voice.as_ref().unwrap().snapshot.duration, 3.0);
    }
}
