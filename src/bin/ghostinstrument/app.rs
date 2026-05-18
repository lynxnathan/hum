use makepad_widgets::*;
use std::sync::{Arc, OnceLock};
use crate::audio::{AudioParams, init_audio_async};

/// Global AudioParams — set once during audio init, read by CanvasWidget.
pub static AUDIO_PARAMS: OnceLock<Arc<AudioParams>> = OnceLock::new();

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::canvas::CanvasWidget;

    App = {{App}} {
        ui: <Window> {
            window: { inner_size: vec2(900, 600) }
            show_bg: true
            draw_bg: { color: #111118 }

            body = <CanvasWidget> {}
        }
    }
}

#[derive(Live)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    _stream: Option<cpal::Stream>,
}

impl LiveHook for App {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        let (stream, params) = init_audio_async();
        self._stream = Some(stream);
        let _ = AUDIO_PARAMS.set(Arc::new(params));
    }
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        makepad_widgets::live_design(cx);
        crate::canvas::live_design(cx);
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

app_main!(App);
