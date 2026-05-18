use makepad_widgets::*;
use makepad_widgets::splitter::SplitterAction;
use std::sync::{Arc, Mutex};
use crate::transport_client::{self, GuiState, FftState};
use crate::visualizer::{VisualizerPreset, set_active_preset};
use crate::shader_editor::ShaderEditorAction;
use crate::key_handler::{self, FocusState, KeysConfig};
use crate::layout_config::LayoutConfig;

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::visualizer::VisualizerView;
    use crate::arrangement_view::ArrangementView;
    use crate::vu_meters::VuMeters;
    use crate::shader_editor::ShaderEditor;
    use crate::terminal_pane::TerminalPane;
    use crate::project_browser::ProjectBrowser;

    // Theme: Catppuccin Mocha (IDE-04) -- switchable via config in future
    DARK_BG   = #1e1e2e  // Base
    MANTLE    = #181825  // Mantle (transport bar bg)
    SURFACE0  = #313244
    SURFACE1  = #45475a
    TEXT_COLOR = #cdd6f4  // Text
    SUBTEXT0  = #a6adc8
    SUBTEXT1  = #bac2de
    ACCENT    = #89b4fa  // Blue
    MAUVE     = #cba6f7
    GREEN     = #a6e3a1
    RED       = #f38ba8
    PEACH     = #fab387
    YELLOW    = #f9e2af
    TEAL      = #94e2d5
    SKY       = #89dceb
    LAVENDER  = #b4befe
    SUBTLE    = #6c7086  // Overlay0

    App = {{App}} {
        ui: <Window> {
            window: { inner_size: vec2(900, 600) }
            show_bg: true
            draw_bg: { color: (DARK_BG) }

            body = <View> {
                flow: Down
                width: Fill
                height: Fill

                // -- Main content: outer horizontal Splitter (browser | main) --
                outer_split = <Splitter> {
                    axis: Horizontal
                    align: FromA(200.0)

                    a = <View> {
                        flow: Down
                        width: Fill
                        height: Fill
                        browser = <ProjectBrowser> {
                            width: Fill
                            height: Fill
                        }
                    }

                    b = <View> {
                        flow: Down
                        width: Fill
                        height: Fill

                        // -- Inner vertical Splitter (top content | bottom terminal) --
                        inner_split = <Splitter> {
                            axis: Vertical
                            align: FromA(420.0)

                            a = <View> {
                                flow: Down
                                width: Fill
                                height: Fill

                                // -- Visualizer (FFT-driven, 4 switchable presets) --
                                visualizer = <VisualizerView> {
                                    width: Fill
                                    height: 220
                                }

                                // -- Shader editor pane (collapsed by default) --
                                shader_editor_wrap = <View> {
                                    width: Fill
                                    height: 0
                                    shader_ed = <ShaderEditor> {}
                                }

                                // -- Arrangement + VU meters (side by side) --
                                mid_zone = <View> {
                                    width: Fill
                                    height: Fill
                                    flow: Right

                                    arrangement = <ArrangementView> {
                                        width: Fill
                                        height: Fill
                                    }

                                    vu_meters = <VuMeters> {
                                        width: 80
                                        height: Fill
                                    }
                                }
                            }

                            b = <View> {
                                flow: Down
                                width: Fill
                                height: Fill

                                // -- Terminal pane (PTY-backed shell) --
                                terminal = <TerminalPane> {
                                    width: Fill
                                    height: Fill
                                }
                            }
                        }
                    }
                }

                // -- Transport bar --
                transport_bar = <View> {
                    width: Fill
                    height: 48
                    show_bg: true
                    draw_bg: { color: (MANTLE) }
                    flow: Right
                    align: { x: 0.0, y: 0.5 }
                    padding: { left: 16, right: 16, top: 8, bottom: 8 }
                    spacing: 12

                    // Status dot
                    status_dot = <View> {
                        width: 12
                        height: 12
                        show_bg: true
                        draw_bg: { color: (SUBTLE) }
                    }

                    play_btn = <Button> {
                        text: "PLAY"
                        draw_text: {
                            text_style: { font_size: 11.0 }
                            color: (TEXT_COLOR)
                        }
                    }

                    stop_btn = <Button> {
                        text: "STOP"
                        draw_text: {
                            text_style: { font_size: 11.0 }
                            color: (TEXT_COLOR)
                        }
                    }

                    // Preset selector buttons
                    preset_wave = <Button> {
                        text: "WAVE"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: (SUBTLE)
                        }
                    }
                    preset_spec = <Button> {
                        text: "SPEC"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: (SUBTLE)
                        }
                    }
                    preset_plasma = <Button> {
                        text: "PLASMA"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: (SUBTLE)
                        }
                    }
                    preset_tunnel = <Button> {
                        text: "TUNNEL"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: (SUBTLE)
                        }
                    }

                    shader_toggle = <Button> {
                        text: "SHADER"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: (SUBTLE)
                        }
                    }

                    pos_label = <Label> {
                        text: "0.00s"
                        draw_text: {
                            text_style: { font_size: 14.0 }
                            color: (ACCENT)
                        }
                    }

                    // Spacer
                    <View> { width: Fill, height: 1 }

                    active_label = <Label> {
                        text: "0 active"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: (SUBTEXT0)
                        }
                    }

                    focus_label = <Label> {
                        text: "[GUI]"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: (MAUVE)
                        }
                    }

                    conn_label = <Label> {
                        text: "disconnected"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: (SUBTLE)
                        }
                    }
                }
            }
        }
    }
}

app_main!(App);

#[derive(Live, LiveHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    gui_state: Arc<Mutex<GuiState>>,
    #[rust]
    fft_state: Arc<Mutex<FftState>>,
    #[rust]
    polling_started: bool,
    #[rust]
    timer: Timer,
    #[rust]
    shader_editor_open: bool,
    #[rust]
    focus: FocusState,
    #[rust]
    keys_cfg: KeysConfig,
    #[rust]
    layout_cfg: LayoutConfig,
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        makepad_widgets::live_design(cx);
        crate::visualizer::live_design(cx);
        crate::arrangement_view::live_design(cx);
        crate::vu_meters::live_design(cx);
        crate::shader_editor::live_design(cx);
        crate::terminal_pane::live_design(cx);
        crate::project_browser::live_design(cx);
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        // Load persisted layout config (used for saving on drag)
        self.layout_cfg = LayoutConfig::load();

        // Initialize project browser with current directory (done via global)
        crate::project_browser::init_project_root(std::env::current_dir().unwrap_or_default());

        // Start the daemon polling thread once
        if !self.polling_started {
            transport_client::start_polling(Arc::clone(&self.gui_state));
            // Initialize global FFT state for VisualizerView, then start polling thread
            crate::visualizer::init_viz_fft_state(Arc::clone(&self.fft_state));
            transport_client::start_fft_polling(Arc::clone(&self.fft_state));
            // Initialize arrangement view state (parses piece.hum for lanes)
            crate::arrangement_view::init_arrangement_state(Arc::clone(&self.gui_state));
            // Initialize VU meters with same state + thing names from parsed lanes
            let thing_names: Vec<String> = crate::arrangement_view::parse_piece_lanes()
                .iter()
                .map(|l| l.name.clone())
                .collect();
            crate::vu_meters::init_vu_state(Arc::clone(&self.gui_state), thing_names);
            self.polling_started = true;
        }
        // Start a repeating timer for UI refresh at ~30fps (shared for transport + spectral + arrangement + VU)
        self.timer = cx.start_interval(0.033);
    }

    fn handle_timer(&mut self, cx: &mut Cx, event: &TimerEvent) {
        if event.timer_id == self.timer.0 {
            // Redraws both transport UI and spectral view (SpectralView reads
            // FFT state from a global OnceLock on each draw_walk call)
            self.update_transport_ui(cx);
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(id!(play_btn)).clicked(actions) {
            transport_client::send_cmd(r#"{"cmd":"play"}"#).ok();
        }
        if self.ui.button(id!(stop_btn)).clicked(actions) {
            transport_client::send_cmd(r#"{"cmd":"stop"}"#).ok();
        }

        // Preset selector buttons — update global atomic, visualizer reads each frame
        if self.ui.button(id!(preset_wave)).clicked(actions) {
            set_active_preset(VisualizerPreset::Waveform);
        }
        if self.ui.button(id!(preset_spec)).clicked(actions) {
            set_active_preset(VisualizerPreset::Spectrum);
        }
        if self.ui.button(id!(preset_plasma)).clicked(actions) {
            set_active_preset(VisualizerPreset::Plasma);
        }
        if self.ui.button(id!(preset_tunnel)).clicked(actions) {
            set_active_preset(VisualizerPreset::Tunnel);
        }

        // Toggle shader editor pane
        if self.ui.button(id!(shader_toggle)).clicked(actions) {
            self.shader_editor_open = !self.shader_editor_open;
            let h = if self.shader_editor_open { 180.0 } else { 0.0 };
            self.ui.view(id!(shader_editor_wrap)).apply_over(cx, live!{ height: (h) });
            // Populate with default source when first opened
            if self.shader_editor_open {
                if self.ui.text_input(id!(shader_input)).text().is_empty() {
                    self.ui.text_input(id!(shader_input))
                        .set_text(cx, &crate::visualizer::default_custom_source());
                }
            }
            self.ui.redraw(cx);
        }

        // Handle ShaderEditorAction::Apply from the shader editor
        for action in actions {
            match action.as_widget_action().cast() {
                ShaderEditorAction::Apply(src) => {
                    match crate::visualizer::reload_shader(&src) {
                        Ok(()) => {
                            self.ui.label(id!(error_label)).set_text(cx, "");
                        }
                        Err(msg) => {
                            self.ui.label(id!(error_label)).set_text(cx, &msg);
                        }
                    }
                }
                ShaderEditorAction::None => {}
            }
        }

        // Handle splitter drag for layout persistence
        for action in actions {
            if let SplitterAction::Changed { axis, align } = action.as_widget_action().cast() {
                match axis {
                    makepad_widgets::splitter::SplitterAxis::Horizontal => {
                        // Outer split (browser width)
                        if let makepad_widgets::splitter::SplitterAlign::FromA(v) = align {
                            self.layout_cfg.browser_width = v;
                        }
                    }
                    makepad_widgets::splitter::SplitterAxis::Vertical => {
                        // Inner split (terminal split position)
                        if let makepad_widgets::splitter::SplitterAlign::FromA(v) = align {
                            self.layout_cfg.terminal_split = v;
                        }
                    }
                }
                self.layout_cfg.save();
            }
        }

        // Trigger a redraw after button actions
        self.update_transport_ui(cx);
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // DAW keyboard shortcuts: dispatch before standard UI handling
        if let Event::KeyDown(key) = event {
            let active = match self.gui_state.lock() {
                Ok(s) => s.active.clone(),
                Err(_) => vec![],
            };
            if let Some(cmd) = key_handler::process_key(key, &mut self.focus, &active, &self.keys_cfg) {
                transport_client::send_cmd(&cmd).ok();
            }
            // When terminal has focus, forward keys to terminal pane
            if self.focus == FocusState::Terminal {
                self.ui.widget(id!(terminal)).handle_event(cx, event, &mut Scope::empty());
                return; // Don't double-dispatch to UI tree
            }
        }

        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

impl App {
    fn update_transport_ui(&mut self, cx: &mut Cx) {
        let state = match self.gui_state.lock() {
            Ok(s) => s.clone(),
            Err(_) => return,
        };

        // Update position label
        let pos_text = format!("{:.2}s", state.pos);
        self.ui.label(id!(pos_label)).set_text(cx, &pos_text);

        // Update connection / status label
        let status_text = if state.connected && state.playing {
            ">>> PLAYING"
        } else if state.connected {
            "|| STOPPED"
        } else {
            "-- OFFLINE"
        };
        self.ui.label(id!(conn_label)).set_text(cx, status_text);

        // Update active thing count
        let active_text = format!("{} active", state.active.len());
        self.ui.label(id!(active_label)).set_text(cx, &active_text);

        // Update focus indicator
        let focus_text = match self.focus {
            FocusState::Gui => "[GUI]",
            FocusState::Terminal => "[TERM]",
        };
        self.ui.label(id!(focus_label)).set_text(cx, focus_text);

        // Sync terminal pane focus flag with our focus state
        // (so cursor blinks correctly)
        // Terminal pane reads has_focus from its own field; we set it via the widget tree
        // Note: TerminalPane.has_focus is set directly via handle_event focus model

        self.ui.redraw(cx);
    }
}
