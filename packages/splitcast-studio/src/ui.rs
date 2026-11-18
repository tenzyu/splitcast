use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use slint::{
    winit_030::{winit::event::WindowEvent, EventResult, WinitWindowAccessor},
    CloseRequestResponse, ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer,
    TimerMode, VecModel,
};
use splitcast_rs_core::Output;
use uuid::Uuid;

use crate::{
    model::{start_args, Chat, Editor, Reader, Stop, SNAPSHOT_INTERVAL},
    network::{Lane, Network, ResultEvent},
    wire::{Call, Failure, Geometry, Layout, Mode, Phase, Reply, Role, SceneAction, Status},
    ChatRow, Diagnostics, StudioWindow,
};

const VIEW_W: f32 = 480.;
const VIEW_H: f32 = 270.;
const RUNNING_STATUS: Duration = Duration::from_secs(1);
const IDLE_STATUS: Duration = Duration::from_secs(2);
const URGENT_STATUS: Duration = Duration::from_millis(250);

enum Control {
    Lifecycle,
    ReadScene,
    ChangeScene(String, SceneAction),
}

#[derive(Clone, Copy)]
enum Guide {
    Camera,
    Overlay,
}

struct Drag {
    guide: Guide,
    pointer: (f32, f32),
    start: (f32, f32),
    scale: f32,
}

struct PreviewGeometry {
    scale: f32,
    left: f32,
    top: f32,
}

impl PreviewGeometry {
    fn new(canvas: &Geometry) -> Self {
        let (width, height) = (canvas.width as f32, canvas.height as f32);
        let scale = (VIEW_W / width).min(VIEW_H / height);
        Self {
            scale,
            left: (VIEW_W - width * scale) / 2.,
            top: (VIEW_H - height * scale) / 2.,
        }
    }

    fn position(&self, x: u32, y: u32) -> (f32, f32) {
        (
            self.left + x as f32 * self.scale,
            self.top + y as f32 * self.scale,
        )
    }
}

#[derive(Default)]
struct WindowState {
    focused: bool,
    occluded: bool,
}

impl WindowState {
    fn observe(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Focused(focused) => self.focused = *focused,
            WindowEvent::Occluded(occluded) => self.occluded = *occluded,
            _ => {}
        }
    }

    fn viewable(&self, shown: bool) -> bool {
        shown && !self.occluded
    }
}

struct Studio {
    network: Network,
    epoch: u64,
    busy: [bool; Lane::ALL.len()],
    control: Option<Control>,
    status: Option<Status>,
    status_error: Option<Failure>,
    status_revision: u64,
    next_status: Instant,
    status_fresh: bool,
    stop: Stop,
    command_note: &'static str,
    mode: Option<(Output, String)>,
    valid_fields: bool,
    editor: Editor,
    read_scene: bool,
    window: WindowState,
    preview: Reader,
    chat_reader: Reader,
    previous_frame: Option<image::RgbaImage>,
    preview_error: bool,
    chat: Chat,
    drag: Option<Drag>,
}

impl Studio {
    fn new(network: Network) -> Self {
        let now = Instant::now();
        Self {
            network,
            epoch: 0,
            busy: [false; Lane::ALL.len()],
            control: None,
            status: None,
            status_error: None,
            status_revision: 0,
            next_status: now,
            status_fresh: false,
            stop: Stop::None,
            command_note: "",
            mode: None,
            valid_fields: true,
            editor: Editor::default(),
            read_scene: true,
            window: WindowState::default(),
            preview: Reader::new(now),
            chat_reader: Reader::new(now),
            previous_frame: None,
            preview_error: false,
            chat: Chat::default(),
            drag: None,
        }
    }

    fn send(&mut self, lane: Lane, call: Call, revision: u64) -> bool {
        let busy = &mut self.busy[lane.index()];
        if *busy || !self.network.send(lane, call, self.epoch, revision) {
            return false;
        }
        *busy = true;
        true
    }

    fn refresh_status(&mut self) {
        self.next_status = Instant::now();
    }

    fn invalidate_status(&mut self) {
        self.status_revision += 1;
        self.status_fresh = false;
        self.refresh_status();
    }

    fn active_phase(&self) -> Option<Phase> {
        self.status
            .as_ref()
            .filter(|_| self.status_fresh)
            .map(|status| status.state)
    }

    fn readers(&mut self, ui: &StudioWindow) -> bool {
        let viewable = self.window.viewable(ui.window().is_visible()) && !self.stop.pending();
        let phase = self.active_phase();
        let preview = self.preview.visibility(
            viewable
                && ui.get_preview_open()
                && ui.get_editor_open()
                && phase == Some(Phase::Running),
        );
        if preview && !self.preview.visible {
            self.clear_snapshot(ui);
            self.preview_error = false;
        }
        let chat = self.chat_reader.visibility(
            viewable
                && ui.get_chat_open()
                && matches!(phase, Some(Phase::Ready | Phase::Running | Phase::Degraded)),
        );
        preview || chat
    }

    fn clear_snapshot(&mut self, ui: &StudioWindow) {
        if self.previous_frame.take().is_some() {
            ui.set_has_snapshot(false);
            ui.set_snapshot(Image::default());
        }
    }

    fn tick(&mut self, results: &flume::Receiver<ResultEvent>, ui: &StudioWindow) {
        let now = Instant::now();
        let mut changed = false;
        while let Ok(result) = results.try_recv() {
            changed |= self.complete(result, ui, now);
        }
        let before = self.stop.label();
        self.stop.observe(None, now);
        changed |= self.readers(ui) || self.stop.label() != before;
        if now >= self.next_status && self.send(Lane::Status, Call::Status, self.status_revision) {
            let delay = if self.stop.pending() {
                URGENT_STATUS
            } else if matches!(self.active_phase(), Some(Phase::Ready | Phase::Running)) {
                RUNNING_STATUS
            } else {
                IDLE_STATUS
            };
            self.next_status = now + delay;
        }
        if self.read_scene && self.scene_phase() && self.send(Lane::Control, Call::Scene, 0) {
            self.control = Some(Control::ReadScene);
            self.read_scene = false;
            changed = true;
        }
        if self.preview.due(now) && self.send(Lane::Preview, Call::Preview, self.preview.revision) {
            self.preview.next = now + SNAPSHOT_INTERVAL;
        }
        if self.chat_reader.due(now) {
            self.send(
                Lane::Chat,
                Call::Chat {
                    epoch: self.chat.epoch.clone(),
                    cursor: self.chat.cursor,
                },
                self.chat_reader.revision,
            );
        }
        if changed {
            self.publish(ui);
        }
    }

    fn complete(&mut self, event: ResultEvent, ui: &StudioWindow, now: Instant) -> bool {
        self.busy[event.lane.index()] = false;
        if event.lane == Lane::Preview {
            // Also space a new request after a stale or canceled screenshot.
            self.preview.next = now + SNAPSHOT_INTERVAL;
        }
        if event.epoch != self.epoch {
            if event.lane == Lane::Control {
                self.control = None;
                self.invalidate_status();
                return true;
            }
            return false;
        }
        match event.lane {
            Lane::Status if event.revision == self.status_revision => {
                self.observe_status(event.result, ui, now)
            }
            Lane::Status => false,
            Lane::Stop => {
                self.stop.accept(
                    event.result.and_then(|reply| match reply {
                        Reply::Stop(reply) => Ok(reply),
                        _ => Err(Failure::Unknown),
                    }),
                    now,
                );
                self.invalidate_status();
                true
            }
            Lane::Control => {
                self.control_result(event.result, ui);
                true
            }
            Lane::Chat if self.chat_reader.accepts(event.revision) => {
                let (changed, delay) =
                    self.chat.update(event.result.and_then(|reply| match reply {
                        Reply::Chat(page) => Ok(page),
                        _ => Err(Failure::Unknown),
                    }));
                self.chat_reader.next = now + delay;
                if changed {
                    self.publish_chat(ui);
                }
                changed
            }
            Lane::Preview if self.preview.accepts(event.revision) => match event.result {
                Ok(Reply::Preview(frame)) if self.previous_frame.as_ref() != Some(&frame) => {
                    let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        frame.as_raw(),
                        frame.width(),
                        frame.height(),
                    );
                    ui.set_snapshot(Image::from_rgba8(pixels));
                    ui.set_has_snapshot(true);
                    self.previous_frame = Some(frame);
                    self.preview_error = false;
                    true
                }
                Ok(Reply::Preview(_)) => false,
                _ => {
                    let changed = !self.preview_error;
                    self.clear_snapshot(ui);
                    self.preview_error = true;
                    changed
                }
            },
            Lane::Chat | Lane::Preview => false,
        }
    }

    fn observe_status(
        &mut self,
        result: Result<Reply, Failure>,
        ui: &StudioWindow,
        now: Instant,
    ) -> bool {
        let previous = self
            .status
            .as_ref()
            .map(|status| (status.role, status.state));
        let was_fresh = self.status_fresh;
        let old_stop = self.stop.label();
        match result {
            Ok(Reply::Status(status)) => {
                self.stop.observe(Some(&status), now);
                self.status_fresh = true;
                if let Some((_, id)) = &self.mode {
                    if status.start_id() != Some(id) {
                        self.mode = None;
                    }
                }
                let had_error = self.status_error.take().is_some();
                let changed = self.status.as_ref() != Some(&status) || had_error || !was_fresh;
                if previous != Some((status.role, status.state)) || !was_fresh {
                    self.read_scene = true;
                }
                if status.state == Phase::Idle
                    && previous.is_some_and(|(_, phase)| phase != Phase::Idle)
                {
                    self.chat = Chat::default();
                    self.publish_chat(ui);
                }
                self.status = Some(status);
                changed || self.stop.label() != old_stop
            }
            error => {
                let failure = error.err().unwrap_or(Failure::Unknown);
                let changed = self.status_error != Some(failure) || was_fresh;
                self.status_error = Some(failure);
                self.status_fresh = false;
                changed
            }
        }
    }

    fn control_result(&mut self, reply: Result<Reply, Failure>, ui: &StudioWindow) {
        match (self.control.take(), reply) {
            (Some(Control::Lifecycle), Ok(Reply::Changed)) => {
                self.command_note = "Command acknowledged · refreshing status";
                self.invalidate_status();
            }
            (Some(Control::Lifecycle), Err(error)) => {
                self.command_note = error.label();
                self.invalidate_status();
            }
            (Some(Control::ReadScene), Ok(Reply::Scene(scene))) => {
                let first_scene = self.editor.scene.is_none();
                self.editor.observe(scene, None);
                if first_scene || !self.editor.dirty {
                    self.sync_fields(ui);
                }
            }
            (Some(Control::ChangeScene(id, action)), Ok(Reply::Scene(scene))) => {
                self.editor.observe(scene, Some((&id, action)));
                if !self.editor.dirty {
                    self.sync_fields(ui);
                }
            }
            (Some(Control::ChangeScene(id, action)), Err(error)) => {
                if error == Failure::Unknown {
                    self.editor.uncertain = Some((id, action));
                }
                if error == Failure::Conflict {
                    self.editor.conflict = true;
                }
                if matches!(error, Failure::Rejected | Failure::Conflict) {
                    // A capture forward can redact a generation rejection.
                    self.editor.needs_observation = true;
                    self.read_scene = true;
                }
                self.editor.notice = Some(error.label());
            }
            (Some(Control::ReadScene), Err(error)) => self.editor.notice = Some(error.label()),
            _ => self.command_note = Failure::Unknown.label(),
        }
    }

    fn lifecycle(&mut self, call: Call) {
        if self.send(Lane::Control, call, 0) {
            self.control = Some(Control::Lifecycle);
            self.stop = Stop::None;
            self.mode = None;
            self.command_note = "Waiting for Agent…";
            self.read_scene = false;
        }
    }

    fn request_stop(&mut self, ui: &StudioWindow) {
        if self.busy[Lane::Stop.index()] {
            return;
        }
        self.epoch = self.network.supersede();
        if let Some(Control::ChangeScene(ref id, action)) = self.control {
            self.editor.uncertain = Some((id.clone(), action));
        }
        self.mode = None;
        self.command_note = "";
        self.stop = Stop::Sending;
        self.drag = None;
        self.invalidate_status();
        self.readers(ui);
        if !self.send(Lane::Stop, Call::Stop, 0) {
            self.stop = Stop::Rejected(Failure::Disconnected);
        }
    }

    fn ready_to_act(&self) -> bool {
        self.status_fresh
            && !self.busy[Lane::Control.index()]
            && !self.stop.pending()
            && self
                .status
                .as_ref()
                .is_some_and(|status| !status.remote_release_pending || status.state != Phase::Idle)
    }

    fn production(&self) -> bool {
        self.status
            .as_ref()
            .is_some_and(|status| status.role == Role::Production)
    }

    fn scene_phase(&self) -> bool {
        !self.stop.pending()
            && self.active_phase().is_some_and(|phase| match phase {
                Phase::Idle => self.production(),
                Phase::Ready | Phase::Running => true,
                _ => false,
            })
    }

    fn scene_actions(&self) -> bool {
        self.ready_to_act()
            && self.scene_phase()
            && self.editor.uncertain.is_none()
            && !self.editor.needs_observation
            && !self.read_scene
    }

    fn scene_change(&mut self, action: SceneAction) {
        if !self.scene_actions() {
            return;
        }
        let Some(scene) = self.editor.scene.as_ref() else {
            return;
        };
        let id = Uuid::new_v4().simple().to_string();
        let request = Call::SceneChange {
            action,
            generation: scene.generation.clone(),
            id: id.clone(),
            layout: (action == SceneAction::Apply).then(|| self.editor.draft.clone()),
        };
        if self.send(Lane::Control, request, 0) {
            self.drag = None;
            self.control = Some(Control::ChangeScene(id, action));
            self.editor.notice = Some("Waiting for Agent scene…");
            self.read_scene = false;
        }
    }

    fn sync_fields(&mut self, ui: &StudioWindow) {
        let draft = &self.editor.draft;
        if let Some(camera) = &draft.camera {
            ui.set_camera_x(camera.x.to_string().into());
            ui.set_camera_y(camera.y.to_string().into());
            ui.set_camera_width(camera.width.to_string().into());
            ui.set_camera_height(camera.height.to_string().into());
        }
        ui.set_overlay_text(draft.overlay.text.as_deref().unwrap_or("").into());
        ui.set_overlay_x(draft.overlay.x.to_string().into());
        ui.set_overlay_y(draft.overlay.y.to_string().into());
        self.valid_fields = true;
    }

    fn editing(&self) -> bool {
        !matches!(self.control, Some(Control::ChangeScene(..)))
    }

    fn input_changed(&mut self, ui: &StudioWindow) {
        if !self.editing() {
            return;
        }
        let bounded = |text: slint::SharedString, max| -> slint::SharedString {
            if text.chars().count() <= max && !text.chars().any(char::is_control) {
                text
            } else {
                text.chars()
                    .filter(|ch| !ch.is_control())
                    .take(max)
                    .collect::<String>()
                    .into()
            }
        };
        let text = bounded(ui.get_overlay_text(), 80);
        if ui.get_overlay_text() != text {
            ui.set_overlay_text(text.clone());
        }
        for (value, set) in [
            (ui.get_camera_x(), StudioWindow::set_camera_x as fn(_, _)),
            (ui.get_camera_y(), StudioWindow::set_camera_y),
            (ui.get_camera_width(), StudioWindow::set_camera_width),
            (ui.get_camera_height(), StudioWindow::set_camera_height),
            (ui.get_overlay_x(), StudioWindow::set_overlay_x),
            (ui.get_overlay_y(), StudioWindow::set_overlay_y),
        ] {
            let value_bounded = bounded(value.clone(), 5);
            if value_bounded != value {
                set(ui, value_bounded);
            }
        }
        let updated: Option<Layout> = (|| {
            let mut draft = self.editor.draft.clone();
            if let Some(camera) = &mut draft.camera {
                camera.x = ui.get_camera_x().parse().ok()?;
                camera.y = ui.get_camera_y().parse().ok()?;
                camera.width = ui.get_camera_width().parse().ok()?;
                camera.height = ui.get_camera_height().parse().ok()?;
            }
            if text.trim().is_empty() {
                draft.overlay = Default::default();
            } else {
                draft.overlay.text = Some(text.to_string());
                draft.overlay.x = ui.get_overlay_x().parse().ok()?;
                draft.overlay.y = ui.get_overlay_y().parse().ok()?;
            }
            Some(draft)
        })();
        self.valid_fields = updated.is_some();
        if let Some(layout) = updated {
            self.editor.edited(layout);
        }
    }

    fn start_args(&self, ui: &StudioWindow) -> Option<serde_json::Value> {
        if !self.ready_to_act() {
            return None;
        }
        let (output, armed_id) = self.mode.as_ref()?;
        start_args(
            self.status.as_ref()?,
            *output,
            Some(armed_id),
            &ui.get_seconds(),
            &ui.get_confirmation(),
        )
    }

    fn guide_at(&self, (x, y): (f32, f32)) -> Option<Drag> {
        if !self.editing() {
            return None;
        }
        let geom = PreviewGeometry::new(&self.editor.scene.as_ref()?.canvas);
        let overlay = &self.editor.draft.overlay;
        let hit = |left, top, width, height| {
            x >= left && x < left + width && y >= top && y < top + height
        };
        let (left, top) = geom.position(overlay.x, overlay.y);
        let (guide, start) = if overlay.text.is_some() && hit(left, top, 38., 20.) {
            (Guide::Overlay, (overlay.x as f32, overlay.y as f32))
        } else {
            let camera = self.editor.draft.camera.as_ref()?;
            let (left, top) = geom.position(camera.x, camera.y);
            if !hit(
                left,
                top,
                camera.width as f32 * geom.scale,
                camera.height as f32 * geom.scale,
            ) {
                return None;
            }
            (Guide::Camera, (camera.x as f32, camera.y as f32))
        };
        Some(Drag {
            guide,
            pointer: (x, y),
            start,
            scale: geom.scale,
        })
    }

    fn drag_to(&mut self, ui: &StudioWindow, x: f32, y: f32) {
        let (Some(drag), Some(scene)) = (&self.drag, &self.editor.scene) else {
            return;
        };
        let x = drag.start.0 + (x - drag.pointer.0) / drag.scale;
        let y = drag.start.1 + (y - drag.pointer.1) / drag.scale;
        let within = |n: f32, extent: u32, bound: u32| {
            n.round().clamp(0., bound.saturating_sub(extent) as f32) as u32
        };
        let mut layout = self.editor.draft.clone();
        match drag.guide {
            Guide::Camera => {
                if let Some(camera) = &mut layout.camera {
                    camera.x = within(x, camera.width, scene.canvas.width);
                    camera.y = within(y, camera.height, scene.canvas.height);
                }
            }
            Guide::Overlay => {
                layout.overlay.x = within(x, 1, scene.canvas.width);
                layout.overlay.y = within(y, 1, scene.canvas.height);
            }
        }
        self.editor.edited(layout);
        self.sync_fields(ui);
    }

    fn publish_chat(&self, ui: &StudioWindow) {
        let messages = self
            .chat
            .messages
            .iter()
            .rev()
            .map(|row| ChatRow {
                name: row.display_name.clone().into(),
                message: row.text.clone().into(),
            })
            .collect::<Vec<_>>();
        ui.set_chat_messages(ModelRc::new(VecModel::from(messages)));
    }

    fn publish(&self, ui: &StudioWindow) {
        let phase = self.active_phase();
        let status = match (&self.status_error, &self.status) {
            (Some(Failure::Unknown), _) => "Agent unavailable · status unconfirmed".into(),
            (Some(error), _) => error.label().into(),
            (None, Some(status)) => {
                let role = if status.role == Role::Capture {
                    "Capture"
                } else {
                    "Production"
                };
                let output = match status.output.as_deref() {
                    Some("recording") => "  ·  Recording",
                    Some("twitch-test") => "  ·  Test",
                    Some("twitch-live") => "  ·  Live",
                    Some(_) => "  ·  Unknown output",
                    None => "",
                };
                format!("{role}  ·  {}{output}", status.state.label())
            }
            _ => "Connecting to Agent".into(),
        };
        ui.set_agent_status(status.into());
        ui.set_command_status(self.stop.label().unwrap_or(self.command_note).into());
        ui.set_stop_pending(self.stop.pending());
        ui.set_production(self.production());
        ui.set_can_prepare(self.ready_to_act() && phase == Some(Phase::Idle));
        ui.set_can_arm(
            self.ready_to_act() && self.status.as_ref().and_then(Status::start_id).is_some(),
        );
        ui.set_can_start(self.start_args(ui).is_some());
        ui.set_armed(match self.mode.as_ref().map(|(out, _)| out) {
            Some(Output::Recording) => 1,
            Some(Output::TwitchTest) => 2,
            Some(Output::TwitchLive) => 3,
            None => 0,
        });

        let preview_status = if !ui.get_preview_open() {
            "Preview hidden"
        } else if !ui.get_editor_open() {
            "Show scene editor to use the preview"
        } else if self.preview_error {
            "OBS snapshot unavailable"
        } else if phase != Some(Phase::Running) {
            "Preview available during an active output"
        } else if !self.preview.visible {
            "Preview paused while the window is inactive"
        } else {
            "Waiting for OBS snapshot"
        };
        ui.set_preview_status(preview_status.into());

        let chat_status = if !ui.get_chat_open() {
            "Chat hidden".to_owned()
        } else if !matches!(phase, Some(Phase::Ready | Phase::Running | Phase::Degraded)) {
            "Chat available during a prepared session".to_owned()
        } else if self.chat.gap() {
            format!("{}  ·  History gap", self.chat.label())
        } else {
            self.chat.label().to_owned()
        };
        ui.set_chat_status(chat_status.into());
        self.publish_scene(ui, phase);
    }

    fn scene_label(&self, phase: Option<Phase>) -> &'static str {
        if self.editor.uncertain.is_none()
            && !self.editor.conflict
            && !self.editor.dirty
            && phase != Some(Phase::Running)
            && self
                .editor
                .scene
                .as_ref()
                .is_some_and(|scene| scene.mode == Mode::Live)
        {
            "Local draft · OBS session inactive"
        } else {
            self.editor.label()
        }
    }

    fn publish_scene(&self, ui: &StudioWindow, phase: Option<Phase>) {
        let scene = self.editor.scene.as_ref();
        let can_change = self.scene_actions();
        let live =
            phase == Some(Phase::Running) && scene.is_some_and(|scene| scene.mode == Mode::Live);
        let applied = scene
            .and_then(|scene| scene.applied.as_ref())
            .filter(|_| live);
        let can_apply =
            can_change && !self.editor.conflict && self.valid_fields && self.editor.valid();
        ui.set_can_read(self.scene_phase() && !self.busy[Lane::Control.index()]);
        ui.set_can_apply(can_apply);
        ui.set_can_reconcile(can_change && live);
        ui.set_can_save(can_change && live && self.editor.can_save());
        ui.set_can_reload(can_change && !self.editor.dirty);
        ui.set_can_use_agent(
            self.editing() && scene.is_some() && (self.editor.dirty || self.editor.conflict),
        );
        ui.set_can_keep_draft(
            self.editing()
                && self.editor.conflict
                && self.editor.valid()
                && !self.editor.needs_observation,
        );
        ui.set_fields_enabled(self.editing());
        ui.set_scene_status(
            if self.status_error.is_some() {
                "Disconnected or unconfirmed · local edits available"
            } else if !self.valid_fields || scene.is_some() && !self.editor.valid() {
                "Draft requires valid canvas coordinates"
            } else {
                self.scene_label(phase)
            }
            .into(),
        );
        ui.set_scene_notice(self.editor.notice.unwrap_or("").into());
        let camera = self.editor.draft.camera.as_ref();
        ui.set_game_info(
            scene
                .map_or_else(
                    || "Fixed".into(),
                    |scene| {
                        let size = scene
                            .sources
                            .as_ref()
                            .map_or(&scene.canvas, |sources| &sources.game);
                        format!("Fixed  ·  {} × {}", size.width, size.height)
                    },
                )
                .into(),
        );
        ui.set_camera(camera.is_some());
        ui.set_camera_visible(camera.is_some_and(|camera| camera.visible));
        ui.set_camera_info(
            applied
                .and_then(|layout| layout.camera.as_ref())
                .map_or_else(
                    || {
                        if scene.is_none() {
                            "Awaiting Agent layout".into()
                        } else if camera.is_none() {
                            "Camera unavailable".into()
                        } else {
                            String::new()
                        }
                    },
                    |c| {
                        format!(
                            "OBS  {}, {}  ·  {} × {}{}",
                            c.x,
                            c.y,
                            c.width,
                            c.height,
                            if c.visible { "" } else { " · hidden" }
                        )
                    },
                )
                .into(),
        );
        ui.set_overlay_info(
            applied
                .map_or_else(String::new, |layout| {
                    let overlay = &layout.overlay;
                    if overlay.text.is_some() {
                        format!("OBS  {}, {}", overlay.x, overlay.y)
                    } else {
                        "OBS  ·  absent".into()
                    }
                })
                .into(),
        );

        let geometry = scene
            .filter(|_| ui.get_editor_open() && ui.get_preview_open())
            .map(|scene| PreviewGeometry::new(&scene.canvas));
        ui.set_preview_guide(geometry.is_some() && camera.is_some());
        let overlay = &self.editor.draft.overlay;
        ui.set_overlay_guide(geometry.is_some() && overlay.text.is_some());
        if let Some(geometry) = geometry {
            if let Some(camera) = camera {
                let (x, y) = geometry.position(camera.x, camera.y);
                ui.set_guide_x(x);
                ui.set_guide_y(y);
                ui.set_guide_w(camera.width as f32 * geometry.scale);
                ui.set_guide_h(camera.height as f32 * geometry.scale);
            }
            let (x, y) = geometry.position(overlay.x, overlay.y);
            ui.set_overlay_guide_x(x);
            ui.set_overlay_guide_y(y);
        }
    }
}

pub(crate) fn run(
    ui: &StudioWindow,
    network: Network,
    results: flume::Receiver<ResultEvent>,
    trace: bool,
) -> Result<(), slint::PlatformError> {
    let state = Rc::new(RefCell::new(Studio::new(network)));
    {
        let mut setup = state.borrow_mut();
        setup.sync_fields(ui);
        setup.publish(ui);
    }

    macro_rules! on {
        ($callback:ident, |$state:ident, $ui:ident $(, $arg:ident)*| $body:block) => {{
            let weak = ui.as_weak();
            let current = state.clone();
            ui.$callback(move |$($arg),*| {
                if let Some($ui) = weak.upgrade() {
                    let mut $state = current.borrow_mut();
                    $body
                    $state.publish(&$ui);
                }
            });
        }};
    }
    on!(on_status, |s, ui| {
        s.refresh_status();
    });
    on!(on_stop, |s, ui| {
        s.request_stop(&ui);
    });
    on!(on_prepare, |s, ui, media| {
        if ui.get_can_prepare() {
            s.lifecycle(Call::Prepare { media });
        }
    });
    on!(on_arm, |s, ui, mode| {
        let output = match mode {
            1 => Output::Recording,
            2 => Output::TwitchTest,
            3 => Output::TwitchLive,
            _ => return,
        };
        if s.ready_to_act() {
            if let Some(id) = s.status.as_ref().and_then(Status::start_id) {
                s.mode = Some((output, id.to_owned()));
                ui.set_seconds("".into());
                ui.set_confirmation("".into());
            }
        }
    });
    {
        let weak = ui.as_weak();
        let current = state.clone();
        ui.on_start_fields_edited(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_can_start(current.borrow().start_args(&ui).is_some());
            }
        });
    }
    on!(on_start, |s, ui| {
        if let Some(args) = s.start_args(&ui) {
            s.lifecycle(Call::Start(args));
        }
    });
    on!(on_scene_edit, |s, ui| {
        s.input_changed(&ui);
    });
    on!(on_read_scene, |s, ui| {
        s.read_scene = true;
    });
    on!(on_apply, |s, ui| {
        if ui.get_can_apply() {
            s.scene_change(SceneAction::Apply);
        }
    });
    on!(on_reconcile, |s, ui| {
        if ui.get_can_reconcile() {
            s.scene_change(SceneAction::Reconcile);
        }
    });
    on!(on_save, |s, ui| {
        if ui.get_can_save() {
            s.scene_change(SceneAction::Save);
        }
    });
    on!(on_reload, |s, ui| {
        if ui.get_can_reload() {
            s.scene_change(SceneAction::Reload);
        }
    });
    on!(on_use_agent, |s, ui| {
        s.editor.replace_with_agent();
        s.sync_fields(&ui);
    });
    on!(on_keep_draft, |s, ui| {
        if ui.get_can_keep_draft() {
            s.editor.conflict = false;
        }
    });
    on!(on_visibility_changed, |s, ui| {
        s.drag = None;
        s.readers(&ui);
    });
    on!(on_camera_toggle, |s, ui| {
        if s.editing() {
            let mut layout = s.editor.draft.clone();
            if let Some(camera) = &mut layout.camera {
                camera.visible = !camera.visible;
            }
            s.editor.edited(layout);
        }
    });
    on!(on_drag_begin, |s, ui, x, y| {
        s.drag = s.guide_at((x, y));
    });
    on!(on_drag_move, |s, ui, x, y| {
        s.drag_to(&ui, x, y);
    });
    on!(on_drag_end, |s, ui| {
        s.drag = None;
    });

    let timer = Timer::default();
    {
        let current = state.clone();
        let weak = ui.as_weak();
        timer.start(TimerMode::Repeated, Duration::from_millis(120), move || {
            if let Some(ui) = weak.upgrade() {
                current.borrow_mut().tick(&results, &ui);
            }
        });
    }
    {
        let current = state.clone();
        let weak = ui.as_weak();
        ui.window().on_close_requested(move || {
            let mut state = current.borrow_mut();
            if state.busy[Lane::Control.index()] || state.stop.pending() {
                state.command_note = "Waiting for the pending control request";
                if let Some(ui) = weak.upgrade() {
                    state.publish(&ui);
                }
                CloseRequestResponse::KeepWindowShown
            } else {
                CloseRequestResponse::HideWindow
            }
        });
    }
    if trace {
        eprintln!("studio-renderer Slint 1.19.0 / winit software");
        ui.global::<Diagnostics>().on_target(|label, x, y| {
            eprintln!("studio-target {label}|{}|{}", x.round(), y.round());
        });
    }
    {
        let weak = ui.as_weak();
        let current = state.clone();
        ui.window().on_winit_window_event(move |_, event| {
            if let Some(ui) = weak.upgrade() {
                let mut state = current.borrow_mut();
                state.window.observe(event);
                match event {
                    WindowEvent::Focused(false) => {
                        state.drag = None;
                    }
                    WindowEvent::Occluded(_) if state.readers(&ui) => {
                        state.publish(&ui);
                    }
                    WindowEvent::RedrawRequested if trace => {
                        let phase = state.active_phase().map(Phase::label).unwrap_or("offline");
                        let stop = state.stop.label().unwrap_or("-");
                        let scene = state.scene_label(state.active_phase());
                        let frame = if state.previous_frame.is_some() {
                            "frame"
                        } else {
                            "-"
                        };
                        eprintln!(
                            "studio-render {phase} | {stop} | {scene} | {frame} | active={} | chat={}",
                            state.window.focused, state.chat.messages.len()
                        );
                        ui.invoke_report_targets();
                    }
                    _ => {}
                }
            }
            EventResult::Propagate
        });
    }
    ui.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_loss_preserves_visible_reads_and_occlusion_invalidates_them() {
        let now = Instant::now();
        let mut window = WindowState::default();
        let mut reader = Reader::new(now);
        assert!(window.viewable(true));
        window.observe(&WindowEvent::Focused(true));
        assert!(reader.visibility(window.viewable(true)));
        let revision = reader.revision;
        reader.next = now + SNAPSHOT_INTERVAL;

        window.observe(&WindowEvent::Focused(false));
        assert!(!window.focused);
        assert!(!reader.visibility(window.viewable(true)));
        assert!(reader.accepts(revision));
        assert!(!reader.due(now));
        assert!(reader.due(now + SNAPSHOT_INTERVAL));

        window.observe(&WindowEvent::Occluded(true));
        assert!(reader.visibility(window.viewable(true)));
        assert!(!reader.accepts(revision));
        assert!(!reader.due(now + SNAPSHOT_INTERVAL));

        window.observe(&WindowEvent::Occluded(false));
        assert!(!window.viewable(false));
        assert!(reader.visibility(window.viewable(true)));
        assert!(!reader.accepts(revision));
    }
}
