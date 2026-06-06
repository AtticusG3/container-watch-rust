//! Slint callbacks and view model (thin layer over core modules).

use crate::config::{exe_directory, load_settings};
use crate::exec;
use crate::models::{ConnectionDraft, ContainerInfo, SshOptions};
use crate::session;
use crate::ssh_docker::{self, SshDockerService};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

slint::include_modules!();

struct AppState {
    ssh: SshDockerService,
    defaults: SshOptions,
    containers: Vec<ContainerInfo>,
    selected_indices: Vec<usize>,
    last_clicked_index: Option<usize>,
    connection: Option<ConnectionDraft>,
    busy: bool,
}

pub fn run_app() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let ui_handle = ui.as_weak();

    let defaults = load_settings(&exe_directory()).ssh;
    let state = Arc::new(Mutex::new(AppState {
        ssh: SshDockerService::new(),
        defaults: defaults.clone(),
        containers: Vec::new(),
        selected_indices: Vec::new(),
        last_clicked_index: None,
        connection: None,
        busy: false,
    }));

    apply_startup_session(&ui, &defaults, state.clone());

    wire_callbacks(&ui, state.clone());

    let ui_handle_close = ui_handle.clone();
    let state_close = state.clone();
    ui.window().on_close_requested(move || {
        if let Some(ui) = ui_handle_close.upgrade() {
            if ui.get_remember_connection() {
                persist_if_remembered(&ui);
            } else {
                session::clear();
            }
        }
        state_close.lock().unwrap().ssh.disconnect();
        slint::CloseRequestResponse::HideWindow
    });

    ui.run()
}

fn wire_callbacks(ui: &AppWindow, state: Arc<Mutex<AppState>>) {
    let ui_handle = ui.as_weak();

    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_connect_clicked(move || on_connect(ui_handle.clone(), state.clone()));
    }
    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_disconnect_clicked(move || on_disconnect(ui_handle.clone(), state.clone()));
    }
    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_refresh_clicked(move || on_refresh(ui_handle.clone(), state.clone()));
    }
    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_restart_all_clicked(move || {
            show_confirm(
                ui_handle.clone(),
                state.clone(),
                "Restart all running".into(),
                "Restart every running container on the remote host?\n\
                 This runs docker ps -q, then docker restart --time 10 on the results."
                    .into(),
                ConfirmAction::RestartAll,
            );
        });
    }
    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_stop_clicked(move || {
            prompt_lifecycle(ui_handle.clone(), state.clone(), LifecycleAction::Stop);
        });
    }
    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_start_clicked(move || {
            prompt_lifecycle(ui_handle.clone(), state.clone(), LifecycleAction::Start);
        });
    }
    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_restart_clicked(move || {
            prompt_lifecycle(ui_handle.clone(), state.clone(), LifecycleAction::Restart);
        });
    }
    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_exec_clicked(move || on_exec(ui_handle.clone(), state.clone()));
    }
    {
        let ui_handle = ui_handle.clone();
        ui.on_remember_changed(move |checked| {
            if !checked {
                session::clear();
            } else if let Some(ui) = ui_handle.upgrade() {
                persist_if_remembered(&ui);
            }
        });
    }
    {
        let ui_handle = ui_handle.clone();
        let state = state.clone();
        ui.on_container_row_clicked(move |index, control, shift| {
            on_row_clicked(
                ui_handle.clone(),
                state.clone(),
                index as usize,
                control,
                shift,
            );
        });
    }
}

enum ConfirmAction {
    RestartAll,
    Lifecycle(LifecycleAction),
}

#[derive(Clone, Copy)]
enum LifecycleAction {
    Stop,
    Start,
    Restart,
}

impl LifecycleAction {
    fn verb(&self) -> &'static str {
        match self {
            Self::Stop => "Stop",
            Self::Start => "Start",
            Self::Restart => "Restart",
        }
    }

    fn progress(&self) -> &'static str {
        match self {
            Self::Stop => "Stopping",
            Self::Start => "Starting",
            Self::Restart => "Restarting",
        }
    }

    fn docker_cli(&self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Start => "start",
            Self::Restart => "restart",
        }
    }

    fn error_title(&self) -> &'static str {
        match self {
            Self::Stop => "Docker stop",
            Self::Start => "Docker start",
            Self::Restart => "Docker restart",
        }
    }

    fn failed_status(&self) -> &'static str {
        match self {
            Self::Stop => "Stop failed.",
            Self::Start => "Start failed.",
            Self::Restart => "Restart failed.",
        }
    }

    fn done_noun(&self) -> &'static str {
        match self {
            Self::Stop => "Stop",
            Self::Start => "Start",
            Self::Restart => "Restart",
        }
    }
}

fn apply_startup_session(ui: &AppWindow, defaults: &SshOptions, state: Arc<Mutex<AppState>>) {
    if let Some(saved) = session::try_load() {
        ui.set_host_text(saved.host.into());
        ui.set_port_text(port_string(saved.port));
        ui.set_user_text(saved.username.into());
        if let Some(pwd) = saved.password {
            ui.set_password_text(pwd.into());
        }
        ui.set_remember_connection(true);

        let ui_weak = ui.as_weak();
        let defaults = defaults.clone();
        let state_timer = state.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(0), move || {
            auto_connect_from_saved(ui_weak, defaults, state_timer);
        });
    } else {
        ui.set_host_text(defaults.host.clone().into());
        ui.set_port_text(port_string(defaults.port));
        ui.set_user_text(defaults.username.clone().into());
        if let Some(pwd) = &defaults.password {
            ui.set_password_text(pwd.clone().into());
        }
        ui.set_status_text(
            "Disconnected. Enter host and credentials, then Connect.".into(),
        );
    }
    sync_ui_from_state(ui, &state.lock().unwrap());
}

fn auto_connect_from_saved(
    ui_weak: slint::Weak<AppWindow>,
    defaults: SshOptions,
    state: Arc<Mutex<AppState>>,
) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };

    let host = ui.get_host_text().to_string();
    let user = ui.get_user_text().to_string();
    let password = effective_password(&ui, &defaults);

    if host.trim().is_empty() || user.trim().is_empty() || password.is_empty() {
        ui.set_status_text(
            "Disconnected. Saved connection needs host, user, and password. Enter missing values and Connect.".into(),
        );
        return;
    }

    let Ok(port) = parse_port(&ui.get_port_text()) else {
        ui.set_status_text("Disconnected. Fix the port, then Connect.".into());
        return;
    };

    run_connect_flow(ui_weak, state, host, port, user, password, defaults);
}

fn on_connect(ui_weak: slint::Weak<AppWindow>, state: Arc<Mutex<AppState>>) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };

    let host = ui.get_host_text().to_string();
    let user = ui.get_user_text().to_string();
    let defaults = state.lock().unwrap().defaults.clone();
    let password = effective_password(&ui, &defaults);

    if host.trim().is_empty() || user.trim().is_empty() || password.is_empty() {
        show_message(
            ui_weak,
            "Connect".into(),
            "Host, user, and password are required.".into(),
        );
        return;
    }

    let port = match parse_port(&ui.get_port_text()) {
        Ok(p) => p,
        Err(msg) => {
            show_message(ui_weak, "Connect".into(), msg.to_string());
            return;
        }
    };

    run_connect_flow(ui_weak, state, host, port, user, password, defaults);
}

fn run_connect_flow(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<Mutex<AppState>>,
    host: String,
    port: u16,
    user: String,
    password: String,
    defaults: SshOptions,
) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };

    {
        let mut st = state.lock().unwrap();
        st.busy = true;
        st.connection = None;
        st.containers.clear();
        st.selected_indices.clear();
    }
    sync_ui_from_state(&ui, &state.lock().unwrap());
    ui.set_status_text("Connecting...".into());

    let timeout = connection_timeout(&defaults);
    let ssh = state.lock().unwrap().ssh.clone();

    let ui_weak2 = ui_weak.clone();
    let state2 = state.clone();
    thread::spawn(move || {
        let connect_result = ssh.connect(&host, port, &user, &password, timeout);

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak2.upgrade() else {
                return;
            };

            match connect_result {
                Ok(()) => {
                    let draft = ConnectionDraft {
                        host: host.clone(),
                        port,
                        username: user.clone(),
                        password,
                        command_timeout_seconds: defaults.command_timeout_seconds,
                    };
                    {
                        let mut st = state2.lock().unwrap();
                        st.connection = Some(draft.clone());
                        st.busy = true;
                    }

                    if ui.get_remember_connection() {
                        let pwd = draft.password.as_str();
                        let _ = session::save(&host, port, &user, Some(pwd));
                    }

                    ui.set_status_text("Connected. Loading containers...".into());
                    sync_ui_from_state(&ui, &state2.lock().unwrap());
                    load_containers(ui_weak2.clone(), state2, true, Some((host, port)));
                }
                Err(err) => {
                    state2.lock().unwrap().ssh.disconnect();
                    {
                        let mut st = state2.lock().unwrap();
                        st.busy = false;
                        st.connection = None;
                    }
                    sync_ui_from_state(&ui, &state2.lock().unwrap());
                    ui.set_status_text("Connection failed.".into());
                    show_message(ui_weak2, "SSH connect".into(), err);
                }
            }
        });
    });
}

fn on_disconnect(ui_weak: slint::Weak<AppWindow>, state: Arc<Mutex<AppState>>) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };

    state.lock().unwrap().ssh.disconnect();
    {
        let mut st = state.lock().unwrap();
        st.connection = None;
        st.containers.clear();
        st.selected_indices.clear();
        st.last_clicked_index = None;
        st.busy = false;
    }
    ui.set_status_text("Disconnected.".into());
    sync_ui_from_state(&ui, &state.lock().unwrap());
}

fn on_refresh(ui_weak: slint::Weak<AppWindow>, state: Arc<Mutex<AppState>>) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };
    if state.lock().unwrap().connection.is_none() {
        return;
    }

    {
        let mut st = state.lock().unwrap();
        st.busy = true;
    }
    sync_ui_from_state(&ui, &state.lock().unwrap());
    ui.set_status_text("Refreshing...".into());
    load_containers(ui_weak, state, false, None);
}

fn load_containers(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<Mutex<AppState>>,
    after_connect: bool,
    connect_label: Option<(String, u16)>,
) {
    let ssh = state.lock().unwrap().ssh.clone();
    let timeout = state
        .lock().unwrap()
        .connection
        .as_ref()
        .map(|c| c.command_timeout())
        .unwrap_or_else(|| connection_timeout(&state.lock().unwrap().defaults));

    thread::spawn(move || {
        let result = ssh.list_containers(timeout);

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };

            match result {
                Ok(list) => {
                    {
                        let mut st = state.lock().unwrap();
                        st.containers = list;
                        st.selected_indices.clear();
                        st.last_clicked_index = None;
                        st.busy = false;
                    }
                    sync_ui_from_state(&ui, &state.lock().unwrap());

                    let count = state.lock().unwrap().containers.len();
                    if after_connect {
                        if let Some((host, port)) = connect_label {
                            ui.set_status_text(
                                format!("Connected to {host}:{port}. {count} container(s).").into(),
                            );
                        }
                    } else {
                        ui.set_status_text(format!("Refreshed. {count} container(s).").into());
                    }
                }
                Err(err) => {
                    {
                        let mut st = state.lock().unwrap();
                        st.busy = false;
                    }
                    sync_ui_from_state(&ui, &state.lock().unwrap());
                    if after_connect {
                        ui.set_status_text("Connection failed.".into());
                    } else {
                        ui.set_status_text("Refresh failed.".into());
                    }
                    show_message(
                        ui_weak,
                        "Docker".into(),
                        err,
                    );
                }
            }
        });
    });
}

fn prompt_lifecycle(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<Mutex<AppState>>,
    action: LifecycleAction,
) {
    let selected = selected_containers(&state.lock().unwrap());
    if selected.is_empty() {
        return;
    }

    let pairs: Vec<(String, String)> = selected
        .iter()
        .map(|c| (c.display_name(), c.restart_target()))
        .collect();
    let refs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(d, t)| (d.as_str(), t.as_str()))
        .collect();
    let msg = ssh_docker::build_action_confirm_message(action.verb(), &refs);

    show_confirm(
        ui_weak,
        state,
        action.verb().into(),
        msg.into(),
        ConfirmAction::Lifecycle(action),
    );
}

fn run_lifecycle(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<Mutex<AppState>>,
    action: LifecycleAction,
) {
    let selected = selected_containers(&state.lock().unwrap());
    if selected.is_empty() {
        return;
    }

    let targets: Vec<String> = selected.iter().map(|c| c.restart_target()).collect();
    let display = selected[0].display_name();
    let count = targets.len();

    {
        let mut st = state.lock().unwrap();
        st.busy = true;
    }
    if let Some(ui) = ui_weak.upgrade() {
        sync_ui_from_state(&ui, &state.lock().unwrap());
        let status = if count == 1 {
            format!("{} {}...", action.progress(), display)
        } else {
            format!(
                "{} {} containers (one docker {})...",
                action.progress(),
                count,
                action.docker_cli()
            )
        };
        ui.set_status_text(status.into());
    }

    let ssh = state.lock().unwrap().ssh.clone();
    let timeout = state
        .lock().unwrap()
        .connection
        .as_ref()
        .map(|c| c.command_timeout())
        .unwrap_or_else(|| connection_timeout(&state.lock().unwrap().defaults));

    let ui_weak2 = ui_weak.clone();
    let state2 = state.clone();
    thread::spawn(move || {
        let result = match action {
            LifecycleAction::Stop => ssh.stop_containers(&targets, timeout),
            LifecycleAction::Start => ssh.start_containers(&targets, timeout),
            LifecycleAction::Restart => ssh.restart_containers(&targets, timeout),
        };

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak2.upgrade() else {
                return;
            };

            match result {
                Ok(()) => {
                    ui.set_status_text("Refreshing...".into());
                    load_containers_after_action(ui_weak2.clone(), state2, action, count);
                }
                Err(err) => {
                    {
                        let mut st = state2.lock().unwrap();
                        st.busy = false;
                    }
                    sync_ui_from_state(&ui, &state2.lock().unwrap());
                    ui.set_status_text(action.failed_status().into());
                    show_message(ui_weak2, action.error_title().into(), err);
                }
            }
        });
    });
}

fn load_containers_after_action(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<Mutex<AppState>>,
    action: LifecycleAction,
    target_count: usize,
) {
    let ssh = state.lock().unwrap().ssh.clone();
    let timeout = state
        .lock().unwrap()
        .connection
        .as_ref()
        .map(|c| c.command_timeout())
        .unwrap_or_else(|| connection_timeout(&state.lock().unwrap().defaults));

    thread::spawn(move || {
        let result = ssh.list_containers(timeout);

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };

            match result {
                Ok(list) => {
                    {
                        let mut st = state.lock().unwrap();
                        st.containers = list;
                        st.selected_indices.clear();
                        st.busy = false;
                    }
                    sync_ui_from_state(&ui, &state.lock().unwrap());
                    let count = state.lock().unwrap().containers.len();
                    ui.set_status_text(
                        format!(
                            "{} issued for {} container(s). {} listed.",
                            action.done_noun(),
                            target_count,
                            count
                        )
                        .into(),
                    );
                }
                Err(err) => {
                    {
                        let mut st = state.lock().unwrap();
                        st.busy = false;
                    }
                    sync_ui_from_state(&ui, &state.lock().unwrap());
                    show_message(ui_weak, "Docker".into(), err);
                }
            }
        });
    });
}

fn run_restart_all(ui_weak: slint::Weak<AppWindow>, state: Arc<Mutex<AppState>>) {
    {
        let mut st = state.lock().unwrap();
        st.busy = true;
    }
    if let Some(ui) = ui_weak.upgrade() {
        sync_ui_from_state(&ui, &state.lock().unwrap());
        ui.set_status_text("Restarting all running containers...".into());
    }

    let ssh = state.lock().unwrap().ssh.clone();
    let timeout = state
        .lock().unwrap()
        .connection
        .as_ref()
        .map(|c| c.command_timeout())
        .unwrap_or_else(|| connection_timeout(&state.lock().unwrap().defaults));

    let ui_weak2 = ui_weak.clone();
    let state2 = state.clone();
    thread::spawn(move || {
        let result = ssh.restart_all_running(timeout);

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak2.upgrade() else {
                return;
            };

            match result {
                Ok(()) => {
                    ui.set_status_text("Refreshing...".into());
                    let ssh = state2.lock().unwrap().ssh.clone();
                    thread::spawn(move || {
                        let list_result = ssh.list_containers(timeout);
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak2.upgrade() else {
                                return;
                            };
                            match list_result {
                                Ok(list) => {
                                    {
                                        let mut st = state2.lock().unwrap();
                                        st.containers = list;
                                        st.selected_indices.clear();
                                        st.busy = false;
                                    }
                                    sync_ui_from_state(&ui, &state2.lock().unwrap());
                                    let count = state2.lock().unwrap().containers.len();
                                    ui.set_status_text(
                                        format!(
                                            "Restart all running completed. {count} container(s) listed."
                                        )
                                        .into(),
                                    );
                                }
                                Err(err) => {
                                    {
                                        let mut st = state2.lock().unwrap();
                                        st.busy = false;
                                    }
                                    sync_ui_from_state(&ui, &state2.lock().unwrap());
                                    show_message(ui_weak2, "Docker".into(), err);
                                }
                            }
                        });
                    });
                }
                Err(err) => {
                    {
                        let mut st = state2.lock().unwrap();
                        st.busy = false;
                    }
                    sync_ui_from_state(&ui, &state2.lock().unwrap());
                    ui.set_status_text("Restart all running failed.".into());
                    show_message(ui_weak2, "Docker restart all".into(), err);
                }
            }
        });
    });
}

fn on_exec(ui_weak: slint::Weak<AppWindow>, state: Arc<Mutex<AppState>>) {
    let (connection, container) = {
        let st = state.lock().unwrap();
        let connection = match st.connection.clone() {
            Some(c) => c,
            None => return,
        };
        let selected = selected_containers(&st);
        if selected.len() != 1 {
            return;
        }
        (connection, selected[0].clone())
    };

    match exec::launch_exec_terminal(
        &connection,
        &container.restart_target(),
        &container.display_name(),
    ) {
        Ok(()) => {}
        Err(err) => show_message(ui_weak, "Exec".into(), err),
    }
}

fn on_row_clicked(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<Mutex<AppState>>,
    index: usize,
    control: bool,
    shift: bool,
) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };

    {
        let mut st = state.lock().unwrap();
        let len = st.containers.len();
        if index >= len {
            return;
        }

        if shift {
            if let Some(anchor) = st.last_clicked_index {
                let start = anchor.min(index);
                let end = anchor.max(index);
                if !control {
                    st.selected_indices.clear();
                }
                for i in start..=end {
                    if !st.selected_indices.contains(&i) {
                        st.selected_indices.push(i);
                    }
                }
            } else {
                st.selected_indices = vec![index];
            }
        } else if control {
            if let Some(pos) = st.selected_indices.iter().position(|&i| i == index) {
                st.selected_indices.remove(pos);
            } else {
                st.selected_indices.push(index);
            }
        } else {
            st.selected_indices = vec![index];
        }
        st.last_clicked_index = Some(index);
    }

    sync_ui_from_state(&ui, &state.lock().unwrap());
}

fn show_message(ui_weak: slint::Weak<AppWindow>, title: SharedString, message: String) {
    let Ok(dialog) = MessageDialog::new() else {
        return;
    };
    dialog.set_dialog_title(title);
    dialog.set_dialog_message(message.into());
    let dialog_weak = dialog.as_weak();
    dialog.on_closed(move || {
        if let Some(d) = dialog_weak.upgrade() {
            let _ = d.hide();
        }
    });
    let _ = ui_weak;
    dialog.show().ok();
}

fn show_confirm(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<Mutex<AppState>>,
    title: SharedString,
    message: SharedString,
    action: ConfirmAction,
) {
    let Ok(dialog) = ConfirmDialog::new() else {
        return;
    };
    dialog.set_dialog_title(title);
    dialog.set_dialog_message(message);

    let dialog_weak_confirm = dialog.as_weak();
    let ui_weak2 = ui_weak.clone();
    let state2 = state.clone();
    dialog.on_confirmed(move || {
        if let Some(d) = dialog_weak_confirm.upgrade() {
            let _ = d.hide();
        }
        match action {
            ConfirmAction::RestartAll => run_restart_all(ui_weak2.clone(), state2.clone()),
            ConfirmAction::Lifecycle(lifecycle) => {
                run_lifecycle(ui_weak2.clone(), state2.clone(), lifecycle);
            }
        }
    });

    let dialog_weak = dialog.as_weak();
    dialog.on_cancelled(move || {
        if let Some(d) = dialog_weak.upgrade() {
            let _ = d.hide();
        }
    });

    if let Some(_ui) = ui_weak.upgrade() {
        dialog.show().ok();
    } else {
        dialog.show().ok();
    }
}

fn selected_containers(st: &AppState) -> Vec<ContainerInfo> {
    st.selected_indices
        .iter()
        .filter_map(|&i| st.containers.get(i).cloned())
        .collect()
}

fn sync_ui_from_state(ui: &AppWindow, st: &AppState) {
    let connected = st.connection.is_some() && st.ssh.is_connected();
    let busy = st.busy;
    let selection_count = st.selected_indices.len();

    ui.set_connected(connected);
    ui.set_busy(busy);
    ui.set_fields_enabled(!busy && !connected);
    ui.set_connect_enabled(!busy && !connected);
    ui.set_disconnect_enabled(!busy && connected);
    ui.set_refresh_enabled(!busy && connected);
    ui.set_restart_all_enabled(!busy && connected);
    let can_act = !busy && connected && selection_count > 0;
    ui.set_stop_enabled(can_act);
    ui.set_start_enabled(can_act);
    ui.set_restart_enabled(can_act);
    ui.set_exec_enabled(!busy && connected && selection_count == 1);

    let rows: Vec<ContainerRow> = st
        .containers
        .iter()
        .enumerate()
        .map(|(i, c)| ContainerRow {
            name: c.display_name().into(),
            state: c.state.clone().unwrap_or_default().into(),
            status: c.status.clone().unwrap_or_default().into(),
            image: c.image.clone().unwrap_or_default().into(),
            id: c.id.clone().unwrap_or_default().into(),
            selected: st.selected_indices.contains(&i),
            restart_target: c.restart_target().into(),
        })
        .collect();

    ui.set_containers(ModelRc::new(Rc::new(VecModel::from(rows))));
}

fn persist_if_remembered(ui: &AppWindow) {
    if !ui.get_remember_connection() {
        session::clear();
        return;
    }
    let host = ui.get_host_text().to_string();
    let user = ui.get_user_text().to_string();
    let Ok(port) = parse_port(&ui.get_port_text()) else {
        return;
    };
    let pwd = ui.get_password_text().to_string();
    let pwd_opt = if pwd.is_empty() { None } else { Some(pwd.as_str()) };
    let _ = session::save(&host, port, &user, pwd_opt);
}

fn effective_password(ui: &AppWindow, defaults: &SshOptions) -> String {
    let ui_pwd = ui.get_password_text().to_string();
    defaults
        .effective_password(&ui_pwd)
        .unwrap_or("")
        .to_string()
}

fn parse_port(text: &str) -> Result<u16, SharedString> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(22);
    }
    let p: u16 = trimmed.parse().map_err(|_| {
        SharedString::from("Port must be a number between 1 and 65535.")
    })?;
    if !(1..=65535).contains(&p) {
        return Err(SharedString::from(
            "Port must be a number between 1 and 65535.",
        ));
    }
    Ok(p)
}

fn port_string(port: u16) -> SharedString {
    if port > 0 {
        port.to_string().into()
    } else {
        "22".into()
    }
}

fn connection_timeout(defaults: &SshOptions) -> std::time::Duration {
    let secs = defaults.command_timeout_seconds.clamp(5, 600);
    std::time::Duration::from_secs(secs)
}