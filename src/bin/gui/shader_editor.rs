use makepad_widgets::*;

/// Actions dispatched from the shader editor UI.
/// Handled in app.rs handle_actions.
#[derive(Clone, Debug, DefaultNone)]
pub enum ShaderEditorAction {
    /// User pressed Apply with the given shader source text.
    Apply(String),
    None,
}

live_design! {
    use link::theme::*;
    use link::widgets::*;

    EDITOR_BG = #11111b
    SUBTLE = #585b70
    TEXT_COLOR = #cdd6f4
    ERROR_COLOR = #f38ba8
    ACCENT = #89b4fa

    pub ShaderEditor = {{ShaderEditor}} {
        width: Fill
        height: 180
        show_bg: true
        draw_bg: { color: (EDITOR_BG) }
        flow: Down
        padding: 0

        // Header row: label + Apply button
        header = <View> {
            width: Fill
            height: 28
            flow: Right
            align: { x: 0.0, y: 0.5 }
            padding: { left: 8, right: 8, top: 4, bottom: 4 }
            spacing: 12
            show_bg: true
            draw_bg: { color: #181825 }

            <Label> {
                text: "Shader Editor"
                draw_text: {
                    text_style: { font_size: 10.0 }
                    color: (SUBTLE)
                }
            }

            // Spacer
            <View> { width: Fill, height: 1 }

            apply_btn = <Button> {
                text: "Apply"
                draw_text: {
                    text_style: { font_size: 10.0 }
                    color: (ACCENT)
                }
            }
        }

        // Text input area for shader source
        shader_input = <TextInput> {
            width: Fill
            height: 130
            ascii_only: false
            empty_message: "// Enter shader parameters..."
            draw_text: {
                text_style: { font_size: 11.0 }
                color: (TEXT_COLOR)
            }
            draw_bg: { color: (EDITOR_BG) }
        }

        // Error display label
        error_label = <Label> {
            width: Fill
            padding: { left: 8, right: 8, top: 2, bottom: 2 }
            draw_text: {
                text_style: { font_size: 10.0 }
                color: (ERROR_COLOR)
            }
            text: ""
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ShaderEditor {
    #[deref]
    view: View,

    #[rust]
    current_src: String,
}

impl Widget for ShaderEditor {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for ShaderEditor {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        if self.button(id!(apply_btn)).clicked(actions) {
            let src = self.text_input(id!(shader_input)).text();
            self.current_src = src.clone();
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                ShaderEditorAction::Apply(src),
            );
        }
    }
}

impl ShaderEditor {
    /// Set the text content of the shader input.
    pub fn set_source(&self, cx: &mut Cx, src: &str) {
        self.text_input(id!(shader_input)).set_text(cx, src);
    }

    /// Show an error message in the error label.
    pub fn show_error(&self, cx: &mut Cx, err: &str) {
        self.label(id!(error_label)).set_text(cx, err);
    }

    /// Clear the error message.
    pub fn clear_error(&self, cx: &mut Cx) {
        self.label(id!(error_label)).set_text(cx, "");
    }
}
