//! Failure-lifecycle regressions for Quick Messages delete and list.
//!
//! Every test drives the production dispatcher (`request_quick_message_delete`,
//! `trigger_quick_messages_fetch`) and the production consumer (`check_events`).
use super::*;
use crate::quick_message::QuickMessage;

const OWNER: &str = "owner_qm_life";
const RUNTIME: usize = 100;

struct Rig {
    app: ZaloGuiApp,
    ctx: egui::Context,
    rx: tokio::sync::mpsc::UnboundedReceiver<WorkerCmd>,
}

fn qm(id: &str, shortcut: &str) -> QuickMessage {
    QuickMessage {
        id: id.into(),
        shortcut: shortcut.into(),
        title: shortcut.trim_start_matches('/').into(),
        text: format!("text of {shortcut}"),
        media: None,
        msg_type: 0,
    }
}

fn rig() -> Rig {
    worker::WORKER_TX.clear();
    let ctx = egui::Context::default();
    let mut app = ZaloGuiApp::from_context(
        &ctx,
        Session::new(false, false, false).unwrap(),
        false,
        vec![],
        false,
    );
    app.is_logged_in = true;
    app.user_uin = OWNER.into();
    app.account_runtime_id = RUNTIME;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    worker::WORKER_TX.set(worker::WorkerSender::new_for_test(tx, ctx.clone()));
    Rig { app, ctx, rx }
}

fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<WorkerCmd>) -> Vec<WorkerCmd> {
    let mut out = Vec::new();
    while let Ok(cmd) = rx.try_recv() {
        out.push(cmd);
    }
    out
}

fn deletes(cmds: &[WorkerCmd]) -> Vec<(u64, Vec<String>)> {
    cmds.iter()
        .filter_map(|c| match c {
            WorkerCmd::DeleteQuickMessages {
                request_id,
                item_ids,
                ..
            } => Some((*request_id, item_ids.clone())),
            _ => None,
        })
        .collect()
}

fn fetches(cmds: &[WorkerCmd]) -> Vec<(u64, u64)> {
    cmds.iter()
        .filter_map(|c| match c {
            WorkerCmd::FetchQuickMessages {
                request_id,
                mutation_epoch,
                ..
            } => Some((*request_id, *mutation_epoch)),
            _ => None,
        })
        .collect()
}

/// Quick-message items carried by every persisted `Save` payload.
fn saves(cmds: &[WorkerCmd]) -> Vec<Vec<QuickMessage>> {
    cmds.iter()
        .filter_map(|c| match c {
            WorkerCmd::Tools {
                command: tools_panel::ToolsCommand::Save(config),
                ..
            } => Some(config.quick_messages.items.clone()),
            _ => None,
        })
        .collect()
}

fn ids(app: &ZaloGuiApp) -> Vec<String> {
    app.tools
        .config
        .quick_messages
        .items
        .iter()
        .map(|i| i.id.clone())
        .collect()
}

fn pump(r: &mut Rig, event: GuiEvent) {
    r.app.event_tx.send(event).unwrap();
    r.app.check_events(&r.ctx);
}

fn delete_ok(request_id: u64, runtime_id: usize) -> GuiEvent {
    GuiEvent::QuickMessagesDeleted {
        owner: OWNER.into(),
        runtime_id,
        request_id,
    }
}

fn delete_failed(request_id: u64, unknown: bool) -> GuiEvent {
    GuiEvent::QuickMessagesDeleteFailed {
        owner: OWNER.into(),
        runtime_id: RUNTIME,
        request_id,
        error: "boom".into(),
        is_transport_unknown: unknown,
    }
}

fn list_failed(request_id: u64, runtime_id: usize) -> GuiEvent {
    GuiEvent::QuickMessagesLoadFailed {
        owner: OWNER.into(),
        runtime_id,
        request_id,
        mutation_epoch: 0,
        error: "network down".into(),
    }
}

fn loaded(request_id: u64, epoch: u64, items: Vec<QuickMessage>) -> GuiEvent {
    GuiEvent::QuickMessagesLoaded {
        owner: OWNER.into(),
        runtime_id: RUNTIME,
        request_id,
        mutation_epoch: epoch,
        items,
        version: 1,
    }
}

// ---------------------------------------------------------------- F1 (delete)

#[test]
fn f1_a_local_item_delete_makes_no_request_and_persists_removal() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("", "/loc"), qm("srv_a", "/a")];

    assert!(r.app.request_quick_message_delete(&qm("", "/loc")));

    let cmds = drain(&mut r.rx);
    assert!(
        deletes(&cmds).is_empty(),
        "local delete must not hit the server"
    );
    assert_eq!(saves(&cmds), vec![vec![qm("srv_a", "/a")]]);
    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
    assert!(r.app.quick_message_pending_deletes.is_empty());
    assert_eq!(r.app.quick_message_mutation_epoch, 0);
}

#[test]
fn f1_b_server_delete_with_absent_worker_keeps_item_and_cache() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    worker::WORKER_TX.clear();

    assert!(!r.app.request_quick_message_delete(&qm("srv_a", "/a")));

    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
    assert!(r.app.quick_message_pending_deletes.is_empty());
    assert!(
        saves(&drain(&mut r.rx)).is_empty(),
        "nothing may be persisted"
    );
}

#[test]
fn f1_c_closed_receiver_keeps_item_and_leaves_no_pending() {
    let Rig {
        mut app,
        ctx: _ctx,
        rx,
    } = rig();
    app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    drop(rx);

    assert!(!app.request_quick_message_delete(&qm("srv_a", "/a")));

    assert_eq!(ids(&app), vec!["srv_a".to_string()]);
    assert!(app.quick_message_pending_deletes.is_empty());
    assert!(!app.quick_message_delete_pending("srv_a"));
}

#[test]
fn f1_d_enqueued_delete_keeps_item_until_ack_and_blocks_duplicate_click() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a"), qm("srv_b", "/b")];

    assert!(r.app.request_quick_message_delete(&qm("srv_b", "/b")));
    let cmds = drain(&mut r.rx);
    let sent = deletes(&cmds);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, vec!["srv_b".to_string()]);
    assert!(saves(&cmds).is_empty(), "no Save before ACK");
    assert_eq!(ids(&r.app), vec!["srv_a".to_string(), "srv_b".to_string()]);
    assert!(r.app.quick_message_delete_pending("srv_b"));

    // Repeated click on the same pending item must not dispatch again.
    assert!(!r.app.request_quick_message_delete(&qm("srv_b", "/b")));
    assert!(deletes(&drain(&mut r.rx)).is_empty());
    assert_eq!(r.app.quick_message_pending_deletes.len(), 1);
}

#[test]
fn f1_e_explicit_rejection_keeps_item_and_allows_retry() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    assert!(r.app.request_quick_message_delete(&qm("srv_a", "/a")));
    let first = deletes(&drain(&mut r.rx))[0].0;

    pump(&mut r, delete_failed(first, false));

    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
    assert!(r.app.quick_message_pending_deletes.is_empty());
    assert!(r.app.quick_message_delete_unknown.is_empty());
    assert_eq!(r.app.quick_message_mutation_epoch, 0);
    assert!(saves(&drain(&mut r.rx)).is_empty());

    assert!(r.app.request_quick_message_delete(&qm("srv_a", "/a")));
    let retry = deletes(&drain(&mut r.rx));
    assert_eq!(retry.len(), 1);
    assert_ne!(retry[0].0, first, "retry needs a fresh request id");
}

#[test]
fn f1_f_success_removes_exact_id_bumps_epoch_once_and_saves_payload() {
    let mut r = rig();
    let mut with_media = qm("srv_c", "/c");
    with_media.media = Some("file://x.png".into());
    with_media.msg_type = 1;
    r.app.tools.config.quick_messages.items =
        vec![qm("srv_a", "/a"), qm("srv_b", "/b"), with_media.clone()];
    assert!(r.app.request_quick_message_delete(&qm("srv_b", "/b")));
    let req = deletes(&drain(&mut r.rx))[0].0;

    pump(&mut r, delete_ok(req, RUNTIME));

    assert_eq!(ids(&r.app), vec!["srv_a".to_string(), "srv_c".to_string()]);
    assert_eq!(r.app.quick_message_mutation_epoch, 1);
    assert!(r.app.quick_message_pending_deletes.is_empty());
    let cmds = drain(&mut r.rx);
    let persisted = saves(&cmds);
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0], vec![qm("srv_a", "/a"), with_media]);
    // The refresh after the mutation carries the new epoch; no second delete is sent.
    assert_eq!(
        fetches(&cmds).iter().map(|f| f.1).collect::<Vec<_>>(),
        vec![1]
    );
    assert!(deletes(&cmds).is_empty());
}

#[test]
fn f1_g_lost_ack_is_unknown_keeps_item_and_never_resends() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    assert!(r.app.request_quick_message_delete(&qm("srv_a", "/a")));
    let req = deletes(&drain(&mut r.rx))[0].0;

    pump(&mut r, delete_failed(req, true));

    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
    assert!(r.app.quick_message_delete_unknown.contains("srv_a"));
    assert!(r.app.quick_message_pending_deletes.is_empty());
    assert_eq!(r.app.quick_message_mutation_epoch, 0);
    let cmds = drain(&mut r.rx);
    assert!(
        deletes(&cmds).is_empty(),
        "unknown must never auto-retry the delete"
    );
    assert!(saves(&cmds).is_empty());
    let readback = fetches(&cmds);
    assert_eq!(readback.len(), 1, "a single read-only readback is allowed");

    // Readback still lists the item: the outcome stays unknown, the item stays.
    pump(&mut r, loaded(readback[0].0, 0, vec![qm("srv_a", "/a")]));
    assert!(r.app.quick_message_delete_unknown.contains("srv_a"));
    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
}

#[test]
fn f1_g2_readback_without_item_resolves_unknown_and_manual_retry_is_available() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a"), qm("srv_b", "/b")];
    assert!(r.app.request_quick_message_delete(&qm("srv_b", "/b")));
    let req = deletes(&drain(&mut r.rx))[0].0;
    pump(&mut r, delete_failed(req, true));
    let readback = fetches(&drain(&mut r.rx));

    pump(&mut r, loaded(readback[0].0, 0, vec![qm("srv_a", "/a")]));

    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
    assert!(r.app.quick_message_delete_unknown.is_empty());

    // A still-unknown item can be deleted again by an explicit user action.
    let mut r2 = rig();
    r2.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    assert!(r2.app.request_quick_message_delete(&qm("srv_a", "/a")));
    let req2 = deletes(&drain(&mut r2.rx))[0].0;
    pump(&mut r2, delete_failed(req2, true));
    drain(&mut r2.rx);
    assert!(r2.app.request_quick_message_delete(&qm("srv_a", "/a")));
    assert_eq!(deletes(&drain(&mut r2.rx)).len(), 1);
    assert!(r2.app.quick_message_delete_unknown.is_empty());
}

#[test]
fn f1_h_duplicate_or_stale_completion_has_no_second_effect() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a"), qm("srv_b", "/b")];
    assert!(r.app.request_quick_message_delete(&qm("srv_b", "/b")));
    let req = deletes(&drain(&mut r.rx))[0].0;
    pump(&mut r, delete_ok(req, RUNTIME));
    assert_eq!(r.app.quick_message_mutation_epoch, 1);
    drain(&mut r.rx);

    // A newer item with the same id appears; the duplicate ACK must not remove it.
    r.app
        .tools
        .config
        .quick_messages
        .items
        .push(qm("srv_b", "/b"));
    pump(&mut r, delete_ok(req, RUNTIME));
    assert_eq!(r.app.quick_message_mutation_epoch, 1);
    assert_eq!(ids(&r.app), vec!["srv_a".to_string(), "srv_b".to_string()]);
    assert!(saves(&drain(&mut r.rx)).is_empty());

    // A completion for an unknown request retires nothing else.
    assert!(r.app.request_quick_message_delete(&qm("srv_a", "/a")));
    drain(&mut r.rx);
    pump(&mut r, delete_ok(req.wrapping_add(9_999), RUNTIME));
    pump(&mut r, delete_failed(req.wrapping_add(9_998), false));
    assert!(r.app.quick_message_delete_pending("srv_a"));
    assert_eq!(r.app.quick_message_mutation_epoch, 1);
}

#[test]
fn f1_i_list_reorder_during_delete_removes_by_id_not_index() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items =
        vec![qm("srv_a", "/a"), qm("srv_b", "/b"), qm("srv_c", "/c")];
    assert!(r.app.request_quick_message_delete(&qm("srv_b", "/b")));
    let req = deletes(&drain(&mut r.rx))[0].0;

    // A fresher list reorders the cache while the delete is still pending.
    assert!(r.app.trigger_quick_messages_fetch());
    let list_id = fetches(&drain(&mut r.rx))[0].0;
    pump(
        &mut r,
        loaded(
            list_id,
            0,
            vec![qm("srv_c", "/c"), qm("srv_b", "/b"), qm("srv_a", "/a")],
        ),
    );
    assert_eq!(
        ids(&r.app),
        vec![
            "srv_c".to_string(),
            "srv_b".to_string(),
            "srv_a".to_string()
        ]
    );

    pump(&mut r, delete_ok(req, RUNTIME));
    assert_eq!(ids(&r.app), vec!["srv_c".to_string(), "srv_a".to_string()]);
}

#[test]
fn f1_j_runtime_replacement_drops_old_delete_completion() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    assert!(r.app.request_quick_message_delete(&qm("srv_a", "/a")));
    let req = deletes(&drain(&mut r.rx))[0].0;

    // Logout/login: lifecycle is reset and the runtime id changes.
    r.app.reset_quick_message_lifecycle();
    r.app.account_runtime_id = RUNTIME + 1;
    assert!(r.app.quick_message_pending_deletes.is_empty());

    pump(&mut r, delete_ok(req, RUNTIME));
    pump(&mut r, delete_failed(req, true));

    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
    assert_eq!(r.app.quick_message_mutation_epoch, 0);
    assert!(r.app.quick_message_delete_unknown.is_empty());
    assert!(saves(&drain(&mut r.rx)).is_empty());

    // Even if the same request id were still pending, the old runtime is fenced off.
    r.app
        .quick_message_pending_deletes
        .insert(req, vec!["srv_a".into()]);
    pump(&mut r, delete_ok(req, RUNTIME));
    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
    assert!(r.app.quick_message_pending_deletes.contains_key(&req));
}

#[test]
fn f1_server_delete_of_unlisted_id_is_ignored() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    assert!(!r.app.request_quick_message_delete(&qm("srv_zzz", "/z")));
    assert!(deletes(&drain(&mut r.rx)).is_empty());
}

// ------------------------------------------------------------------ F2 (list)

#[test]
fn f2_a_list_failure_retires_in_flight_and_keeps_cache() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    assert!(r.app.trigger_quick_messages_fetch());
    let a = r.app.quick_message_in_flight_list_id.unwrap();
    drain(&mut r.rx);

    pump(&mut r, list_failed(a, RUNTIME));

    assert!(r.app.quick_message_in_flight_list_id.is_none());
    assert!(!r.app.quick_message_refresh_pending);
    assert_eq!(r.app.quick_message_mutation_epoch, 0);
    assert_eq!(
        r.app.quick_message_list_error.as_deref(),
        Some("network down")
    );
    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
    assert!(
        saves(&drain(&mut r.rx)).is_empty(),
        "a failure must not persist"
    );
}

#[test]
fn f2_b_retry_after_failure_enqueues_a_new_request() {
    let mut r = rig();
    assert!(r.app.trigger_quick_messages_fetch());
    let a = r.app.quick_message_in_flight_list_id.unwrap();
    drain(&mut r.rx);
    pump(&mut r, list_failed(a, RUNTIME));

    assert!(
        r.app.trigger_quick_messages_fetch(),
        "retry must really enqueue"
    );
    let cmds = drain(&mut r.rx);
    let sent = fetches(&cmds);
    assert_eq!(sent.len(), 1);
    assert_ne!(sent[0].0, a);
    assert_eq!(r.app.quick_message_in_flight_list_id, Some(sent[0].0));
    assert!(r.app.quick_message_list_error.is_none());
}

#[test]
fn f2_c_stale_failure_does_not_retire_newer_request() {
    let mut r = rig();
    assert!(r.app.trigger_quick_messages_fetch());
    let a = r.app.quick_message_in_flight_list_id.unwrap();
    pump(&mut r, list_failed(a, RUNTIME));
    assert!(r.app.trigger_quick_messages_fetch());
    let b = r.app.quick_message_in_flight_list_id.unwrap();
    assert_ne!(a, b);
    r.app.status_msg = "untouched".into();

    // The older request reports its failure again after B is pending.
    pump(&mut r, list_failed(a, RUNTIME));

    assert_eq!(r.app.quick_message_in_flight_list_id, Some(b));
    assert!(r.app.quick_message_list_error.is_none());
    assert_eq!(r.app.status_msg, "untouched");
}

#[test]
fn f2_e_mutation_during_pending_list_survives_list_failure_without_loop() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    assert!(r.app.trigger_quick_messages_fetch());
    let a = r.app.quick_message_in_flight_list_id.unwrap();
    drain(&mut r.rx);

    let mut created = qm("srv_new", "/new");
    created.media = Some("file://voice.ogg".into());
    created.msg_type = 2;
    pump(
        &mut r,
        GuiEvent::QuickMessageCreated {
            owner: OWNER.into(),
            runtime_id: RUNTIME,
            request_id: 7_001,
            item: created.clone(),
        },
    );
    assert!(
        r.app.quick_message_refresh_pending,
        "refresh is coalesced behind A"
    );
    let persisted = saves(&drain(&mut r.rx));
    assert_eq!(persisted.len(), 1);
    assert!(persisted[0].contains(&created));

    pump(&mut r, list_failed(a, RUNTIME));

    assert!(r.app.quick_message_in_flight_list_id.is_none());
    assert!(
        !r.app.quick_message_refresh_pending,
        "flag must not stay stuck"
    );
    let cmds = drain(&mut r.rx);
    assert!(
        fetches(&cmds).is_empty(),
        "no immediate refetch loop on failure"
    );
    assert!(saves(&cmds).is_empty());
    let kept = r
        .app
        .tools
        .config
        .quick_messages
        .items
        .iter()
        .find(|i| i.id == "srv_new")
        .expect("created item must survive");
    assert_eq!(kept, &created);
    assert_eq!(r.app.quick_message_mutation_epoch, 1);
}

#[test]
fn f2_g_closed_channel_leaves_no_pending_or_loading_state() {
    let Rig {
        mut app,
        ctx: _ctx,
        rx,
    } = rig();
    drop(rx);
    assert!(!app.trigger_quick_messages_fetch());
    assert!(app.quick_message_in_flight_list_id.is_none());
    assert!(!app.quick_message_refresh_pending);
    assert!(app.quick_message_list_error.is_some());

    worker::WORKER_TX.clear();
    assert!(!app.trigger_quick_messages_fetch());
    assert!(app.quick_message_in_flight_list_id.is_none());
}

#[test]
fn f2_h_failure_from_previous_runtime_does_not_touch_new_runtime() {
    let mut r = rig();
    assert!(r.app.trigger_quick_messages_fetch());
    let old = r.app.quick_message_in_flight_list_id.unwrap();
    drain(&mut r.rx);

    // Logout/login as the same account: new runtime, fresh lifecycle.
    r.app.reset_quick_message_lifecycle();
    r.app.account_runtime_id = RUNTIME + 1;
    assert!(r.app.trigger_quick_messages_fetch());
    let fresh = r.app.quick_message_in_flight_list_id.unwrap();

    pump(&mut r, list_failed(old, RUNTIME));
    assert_eq!(r.app.quick_message_in_flight_list_id, Some(fresh));
    assert!(r.app.quick_message_list_error.is_none());

    // Even a colliding request id from the old runtime is fenced by runtime id.
    pump(&mut r, list_failed(fresh, RUNTIME));
    assert_eq!(r.app.quick_message_in_flight_list_id, Some(fresh));
}

#[test]
fn f2_success_after_failure_keeps_epoch_and_order_guards() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    assert!(r.app.trigger_quick_messages_fetch());
    let a = r.app.quick_message_in_flight_list_id.unwrap();
    pump(&mut r, list_failed(a, RUNTIME));
    assert!(r.app.trigger_quick_messages_fetch());
    let b = r.app.quick_message_in_flight_list_id.unwrap();

    pump(
        &mut r,
        loaded(b, 0, vec![qm("srv_a", "/a"), qm("srv_d", "/d")]),
    );
    assert_eq!(r.app.quick_message_latest_applied_list_id, b);
    assert!(r.app.quick_message_list_error.is_none());
    assert_eq!(ids(&r.app).len(), 2);

    // An older snapshot arriving later is still dropped by the order guard.
    pump(&mut r, loaded(a, 0, vec![]));
    assert_eq!(ids(&r.app).len(), 2);
}

// ------------------------------------------------- cache hydration / state revision

use super::tools_panel::{ToolsCommand, ToolsOrigin, ToolsResult};
use crate::tools::Preferences;

fn prefs(items: Vec<QuickMessage>) -> Preferences {
    let mut p = Preferences::default();
    p.quick_messages.items = items;
    p
}

/// Dispatch a production cache load and return the identity the worker received.
fn dispatch_load(r: &mut Rig) -> ToolsOrigin {
    assert!(r.app.tools_command(ToolsCommand::Load));
    let mut found = None;
    for cmd in drain(&mut r.rx) {
        if let WorkerCmd::Tools {
            origin,
            command: ToolsCommand::Load,
            ..
        } = cmd
        {
            found = origin;
        }
    }
    found.expect("a config-bearing command must carry its dispatch identity")
}

/// Complete a Tools command the way the worker does: stamp identity, then deliver.
fn complete_tools(r: &mut Rig, origin: Option<ToolsOrigin>, config: Preferences, paused: bool) {
    let mut result = Ok(ToolsResult::Loaded {
        config,
        tasks: vec![],
        paused,
        origin: None,
    });
    worker::stamp_tools_origin(&mut result, origin);
    pump(
        r,
        GuiEvent::Tools {
            owner: OWNER.into(),
            result,
        },
    );
}

fn save_payloads(cmds: &[WorkerCmd]) -> Vec<Preferences> {
    cmds.iter()
        .filter_map(|c| match c {
            WorkerCmd::Tools {
                command: ToolsCommand::Save(config),
                ..
            } => Some(config.clone()),
            _ => None,
        })
        .collect()
}

fn db_with(initial: Vec<QuickMessage>) -> crate::store::MessageStore {
    let store = crate::store::MessageStore::open(":memory:").unwrap();
    crate::tools::save_preferences(&store, prefs(initial)).unwrap();
    store
}

fn db_items(store: &crate::store::MessageStore) -> Vec<QuickMessage> {
    crate::tools::preferences(store)
        .unwrap()
        .quick_messages
        .items
}

fn apply_saves(store: &crate::store::MessageStore, payloads: &[Preferences]) {
    for p in payloads {
        crate::tools::save_preferences(store, p.clone()).unwrap();
    }
}

fn server_list(r: &mut Rig, items: Vec<QuickMessage>) -> Vec<WorkerCmd> {
    assert!(r.app.trigger_quick_messages_fetch());
    let list_id = fetches(&drain(&mut r.rx))[0].0;
    let epoch = r.app.quick_message_mutation_epoch;
    pump(r, loaded(list_id, epoch, items));
    drain(&mut r.rx)
}

#[test]
fn h1_server_list_applied_while_cache_load_pending_is_not_overwritten() {
    let mut r = rig();
    let old = qm("srv_old", "/old");
    let fresh = qm("srv_new", "/new");
    let store = db_with(vec![old.clone()]);
    r.app.tools.config.quick_messages.items = vec![old.clone()];

    // Cache load A reads the old DB snapshot at dispatch time.
    let origin_a = dispatch_load(&mut r);
    let snapshot_a = crate::tools::preferences(&store).unwrap();
    assert_eq!(
        r.app.quick_message_mutation_epoch, 0,
        "no mutation involved"
    );

    // Server list B is valid and applied first (mutation epoch still 0).
    let cmds = server_list(&mut r, vec![fresh.clone()]);
    apply_saves(&store, &save_payloads(&cmds));
    assert_eq!(r.app.quick_message_mutation_epoch, 0);
    assert_eq!(ids(&r.app), vec!["srv_new".to_string()]);

    // A arrives late with the old snapshot.
    complete_tools(&mut r, Some(origin_a), snapshot_a, false);

    assert_eq!(ids(&r.app), vec!["srv_new".to_string()]);
    let after = drain(&mut r.rx);
    assert!(
        save_payloads(&after).is_empty(),
        "stale A must not be persisted"
    );
    assert_eq!(db_items(&store), vec![fresh]);
}

fn local_edit_then_late_cache_load(
    edit: impl FnOnce(&mut Rig),
    seed: Vec<QuickMessage>,
) -> (Rig, Vec<Preferences>, Preferences) {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = seed.clone();
    let store = db_with(seed);
    let origin_a = dispatch_load(&mut r);
    let snapshot_a = crate::tools::preferences(&store).unwrap();
    edit(&mut r);
    let saves_after_edit = save_payloads(&drain(&mut r.rx));
    complete_tools(&mut r, Some(origin_a), snapshot_a.clone(), false);
    (r, saves_after_edit, snapshot_a)
}

#[test]
fn h2_local_create_during_cache_load_is_not_lost() {
    let (mut r, saved, _) = local_edit_then_late_cache_load(
        |r| {
            assert!(r.app.add_quick_message_local("/n", "N", "new text"));
        },
        vec![qm("srv_a", "/a")],
    );
    assert!(r
        .app
        .tools
        .config
        .quick_messages
        .items
        .iter()
        .any(|i| i.shortcut == "/n" && i.text == "new text"));
    assert!(save_payloads(&drain(&mut r.rx)).is_empty());
    let store = db_with(vec![]);
    apply_saves(&store, &saved);
    assert!(db_items(&store).iter().any(|i| i.shortcut == "/n"));
}

#[test]
fn h2_local_update_during_cache_load_is_not_reverted() {
    let (mut r, saved, _) = local_edit_then_late_cache_load(
        |r| {
            assert!(r.app.add_quick_message_local("/a", "A", "edited text"));
        },
        vec![qm("", "/a")],
    );
    assert_eq!(
        r.app.tools.config.quick_messages.expand("/a"),
        Some("edited text")
    );
    assert!(save_payloads(&drain(&mut r.rx)).is_empty());
    let store = db_with(vec![]);
    apply_saves(&store, &saved);
    assert_eq!(db_items(&store)[0].text, "edited text");
}

#[test]
fn h2_local_delete_during_cache_load_is_not_resurrected() {
    let (mut r, saved, _) = local_edit_then_late_cache_load(
        |r| {
            assert!(r.app.request_quick_message_delete(&qm("", "/loc")));
        },
        vec![qm("", "/loc")],
    );
    assert!(ids(&r.app).is_empty());
    assert!(r.app.tools.config.quick_messages.items.is_empty());
    assert!(save_payloads(&drain(&mut r.rx)).is_empty());
    let store = db_with(vec![qm("", "/loc")]);
    apply_saves(&store, &saved);
    assert!(db_items(&store).is_empty());
}

#[test]
fn h3_server_mutation_during_cache_load_keeps_mutation_payload() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    let store = db_with(vec![qm("srv_a", "/a")]);
    let origin_a = dispatch_load(&mut r);
    let snapshot_a = crate::tools::preferences(&store).unwrap();

    let mut created = qm("srv_n", "/n");
    created.media = Some("file://voice.ogg".into());
    created.msg_type = 2;
    pump(
        &mut r,
        GuiEvent::QuickMessageCreated {
            owner: OWNER.into(),
            runtime_id: RUNTIME,
            request_id: 8_001,
            item: created.clone(),
        },
    );
    let cmds = drain(&mut r.rx);
    apply_saves(&store, &save_payloads(&cmds));

    complete_tools(&mut r, Some(origin_a), snapshot_a, false);

    assert!(r.app.tools.config.quick_messages.items.contains(&created));
    assert!(save_payloads(&drain(&mut r.rx)).is_empty());
    assert!(db_items(&store).contains(&created));
}

#[test]
fn h4_cache_loads_returning_out_of_order_do_not_roll_state_back() {
    // (i) The newer load changes state: the older one is rejected by the revision.
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_x", "/x")];
    let a = dispatch_load(&mut r);
    let b = dispatch_load(&mut r);
    assert!(b.request_id > a.request_id);
    complete_tools(&mut r, Some(b), prefs(vec![qm("srv_b", "/b")]), false);
    assert_eq!(ids(&r.app), vec!["srv_b".to_string()]);
    complete_tools(&mut r, Some(a), prefs(vec![qm("srv_a", "/a")]), false);
    assert_eq!(ids(&r.app), vec!["srv_b".to_string()]);

    // (ii) The newer load leaves state unchanged (no revision bump): order guard still drops A.
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_x", "/x")];
    let a = dispatch_load(&mut r);
    let b = dispatch_load(&mut r);
    let revision_before = r.app.quick_message_state_revision;
    complete_tools(&mut r, Some(b), prefs(vec![qm("srv_x", "/x")]), false);
    assert_eq!(r.app.quick_message_state_revision, revision_before);
    complete_tools(&mut r, Some(a), prefs(vec![qm("srv_a", "/a")]), false);
    assert_eq!(ids(&r.app), vec!["srv_x".to_string()]);
}

#[test]
fn h5_normal_cache_load_hydrates_and_does_not_disable_later_loads() {
    let mut r = rig();
    let origin = dispatch_load(&mut r);
    let before = r.app.quick_message_state_revision;
    complete_tools(
        &mut r,
        Some(origin),
        prefs(vec![qm("srv_a", "/a"), qm("", "/loc")]),
        false,
    );
    assert_eq!(ids(&r.app), vec!["srv_a".to_string(), String::new()]);
    assert_eq!(
        r.app.quick_message_state_revision,
        before + 1,
        "hydration changed state"
    );
    assert!(
        save_payloads(&drain(&mut r.rx)).is_empty(),
        "hydration never persists"
    );

    // A load dispatched after hydration captures the new revision and is accepted too.
    let next = dispatch_load(&mut r);
    complete_tools(&mut r, Some(next), prefs(vec![qm("srv_a", "/a")]), false);
    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);
}

#[test]
fn h6_duplicate_or_stale_completion_changes_nothing() {
    let mut r = rig();
    let origin = dispatch_load(&mut r);
    complete_tools(&mut r, Some(origin), prefs(vec![qm("srv_a", "/a")]), false);
    let revision = r.app.quick_message_state_revision;

    // The same completion delivered again must have no effect.
    r.app
        .tools
        .config
        .quick_messages
        .items
        .push(qm("srv_z", "/z"));
    complete_tools(&mut r, Some(origin), prefs(vec![qm("srv_a", "/a")]), false);
    assert_eq!(r.app.quick_message_state_revision, revision);
    assert!(ids(&r.app).contains(&"srv_z".to_string()));

    // Failures and stale responses never bump the revision. First apply a genuine list so
    // that an older list id is really stale.
    let _ = server_list(&mut r, vec![qm("srv_a", "/a"), qm("srv_z", "/z")]);
    let revision = r.app.quick_message_state_revision;
    let applied = r.app.quick_message_latest_applied_list_id;
    assert!(r.app.request_quick_message_delete(&qm("srv_a", "/a")));
    let req = deletes(&drain(&mut r.rx))[0].0;
    pump(&mut r, delete_failed(req, false));
    pump(&mut r, delete_ok(req, RUNTIME));
    assert!(r.app.trigger_quick_messages_fetch());
    let list = fetches(&drain(&mut r.rx))[0].0;
    pump(&mut r, list_failed(list, RUNTIME));
    pump(&mut r, loaded(applied, 0, vec![]));
    assert_eq!(r.app.quick_message_state_revision, revision);
}

#[test]
fn h7_completion_from_previous_runtime_never_hydrates_new_runtime() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    let origin = dispatch_load(&mut r);

    // Logout/login as the same account: reset plus a new runtime id.
    r.app.reset_quick_message_lifecycle();
    r.app.account_runtime_id = RUNTIME + 1;
    complete_tools(
        &mut r,
        Some(origin),
        prefs(vec![qm("srv_old", "/old")]),
        false,
    );
    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);

    // Even with an equal revision the runtime id alone fences the completion off.
    r.app.quick_message_state_revision = origin.quick_message_revision;
    r.app.quick_message_latest_cache_load_id = 0;
    complete_tools(
        &mut r,
        Some(origin),
        prefs(vec![qm("srv_old", "/old")]),
        false,
    );
    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);

    // The new runtime's own load hydrates normally.
    let fresh = dispatch_load(&mut r);
    assert_eq!(fresh.runtime_id, RUNTIME + 1);
    complete_tools(&mut r, Some(fresh), prefs(vec![qm("srv_b", "/b")]), false);
    assert_eq!(ids(&r.app), vec!["srv_b".to_string()]);
}

#[test]
fn h8_stale_quick_messages_do_not_block_other_config_groups() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];
    let origin = dispatch_load(&mut r);
    let _ = server_list(&mut r, vec![qm("srv_new", "/new")]);

    let mut config = prefs(vec![qm("srv_old", "/old")]);
    config.theme = serde_json::json!({"mode": "from-cache"});
    complete_tools(&mut r, Some(origin), config, true);

    assert_eq!(ids(&r.app), vec!["srv_new".to_string()], "QM stays current");
    assert_eq!(
        r.app.tools.config.theme,
        serde_json::json!({"mode": "from-cache"}),
        "other groups still follow the normal policy"
    );
    assert!(r.app.tools.paused);
    assert!(save_payloads(&drain(&mut r.rx)).is_empty());
}

#[test]
fn h9_unstamped_snapshot_and_save_echo_cannot_overwrite_quick_messages() {
    let mut r = rig();
    r.app.tools.config.quick_messages.items = vec![qm("srv_a", "/a")];

    // Unstamped results (no dispatch identity) fail closed.
    complete_tools(&mut r, None, prefs(vec![]), false);
    assert_eq!(ids(&r.app), vec!["srv_a".to_string()]);

    // A Save echo (Loaded) from before a later local edit is stale.
    assert!(r
        .app
        .tools_command(ToolsCommand::Save(r.app.tools.config.clone())));
    let mut save_origin = None;
    let mut payload = None;
    for cmd in drain(&mut r.rx) {
        if let WorkerCmd::Tools {
            origin,
            command: ToolsCommand::Save(config),
            ..
        } = cmd
        {
            save_origin = origin;
            payload = Some(config);
        }
    }
    assert!(r.app.add_quick_message_local("/n", "N", "later local edit"));
    let later_saves = save_payloads(&drain(&mut r.rx));
    complete_tools(&mut r, save_origin, payload.unwrap(), false);
    assert!(r
        .app
        .tools
        .config
        .quick_messages
        .items
        .iter()
        .any(|i| i.shortcut == "/n"));
    assert!(save_payloads(&drain(&mut r.rx)).is_empty());
    let store = db_with(vec![]);
    apply_saves(&store, &later_saves);
    assert!(db_items(&store).iter().any(|i| i.shortcut == "/n"));
}
