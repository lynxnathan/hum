---
phase: 14-milkdrop-viz
plan: 2
type: execute
wave: 2
depends_on: [14-PLAN-1]
files_modified:
  - src/bin/gui/shader_editor.rs
  - src/bin/gui/app.rs
autonomous: true
requirements: [VIZ-03]

must_haves:
  truths:
    - "A text area inside the GUI shows the current preset's shader source"
    - "Editing the shader text and pressing Apply updates the visualization without restarting"
    - "A compile error in the shader shows an error message in the pane — the old shader keeps running"
    - "The editor pane can be toggled open/closed without losing edits"
  artifacts:
    - path: "src/bin/gui/shader_editor.rs"
      provides: "ShaderEditor widget with TextInput, Apply button, error display"
      exports: [ShaderEditor]
    - path: "src/bin/gui/app.rs"
      provides: "ShaderEditor wired below visualizer, toggle button in transport bar"
      contains: "shader_editor"
  key_links:
    - from: "src/bin/gui/shader_editor.rs Apply button"
      to: "src/bin/gui/visualizer.rs reload_shader()"
      via: "ShaderEditorAction::Apply(shader_src) dispatched via cx.widget_action"
      pattern: "ShaderEditorAction"
    - from: "src/bin/gui/visualizer.rs"
      to: "live_design! shader source string"
      via: "Makepad live_design re-parse or DrawShader fragment swap"
      pattern: "reload_shader"
---

<objective>
Add a shader editor pane beneath the visualizer. Users can edit the current preset's
fragment shader source in a text area, hit Apply, and see the visualization update live.
Compile errors surface inline — the visualizer keeps running on the last good shader.

Purpose: Live shader coding experience (VIZ-03).
Output: shader_editor.rs widget + toggle wired into app.rs.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/14-milkdrop-viz/14-1-SUMMARY.md

<interfaces>
<!-- From 14-PLAN-1 output — VisualizerView public API -->
```rust
// In src/bin/gui/visualizer.rs
pub fn set_preset(&mut self, cx: &mut Cx, preset: VisualizerPreset);

// New method to add in this plan:
// pub fn reload_shader(&mut self, cx: &mut Cx, fragment_src: &str) -> Result<(), String>
// Attempts to update the live shader. Returns Err(msg) on compile failure.
// On success, self.custom_shader_src = Some(fragment_src.to_string()), redraw.
// On failure, keeps previous shader, returns error text.
```

<!-- Makepad live reload mechanism -->
// Makepad's live_design! is re-parsed by the framework at dev time.
// For runtime shader hot-swap, the approach is:
// 1. Store the fragment pixel() function body as a String field in VisualizerView.
// 2. When reloading, construct a new live_design! string dynamically and call
//    Cx::live_design_update() if available, OR
// 3. Simpler fallback: swap the shader source string field that pixel() reads from
//    a global/mutex, switching to a "custom" branch in the preset enum.
// Recommended approach: add VisualizerPreset::Custom(String) variant and a
// custom pixel() branch that uses a `uniform float[256] custom_code` trick is NOT viable.
// ACTUAL approach: keep a `custom_fragment: Option<String>`, compile it by constructing
// a new DrawShader via live_design string injection using `cx.live_ptr_from_id` and
// `LiveId` lookup — OR simply rewrite as a file-based hot-reload:
// Write the edited shader to /tmp/hum_shader_custom.glsl and call
// `std::process::Command::new("touch").arg(file)` to trigger Makepad's file watcher.
// SIMPLEST viable approach: store the custom pixel() body in a static Mutex<String>,
// always call it in the VisualizerView::pixel() via a uniform that selects custom mode.
// The executor should pick the simplest approach that compiles and demonstrates live update.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: ShaderEditor widget + VisualizerView reload_shader</name>
  <files>src/bin/gui/shader_editor.rs, src/bin/gui/visualizer.rs</files>
  <action>
**Part A — Add reload_shader to visualizer.rs:**

1. Add `pub enum VisualizerPreset { Waveform, Spectrum, Plasma, Tunnel, Custom }`.

2. Add `#[rust] custom_pixel_src: String` field to VisualizerView (default = empty string).
   Add `#[rust] shader_error: Option<String>` field.

3. Add `pub fn reload_shader(&mut self, cx: &mut Cx, pixel_src: &str) -> Result<(), String>`:
   - Store `pixel_src` in `self.custom_pixel_src`
   - Set `self.preset = VisualizerPreset::Custom`
   - Write the full shader source to `/tmp/hum_viz_custom.glsl` for debugging reference
   - The custom preset branch in `pixel()` reads a uniform array `uniform bass/mids/highs/time`
     and evaluates a hardcoded "user edits this GLSL-like expression" interpretation.
   - **Practical approach**: The executor must pick between:
     a) Injecting the custom src as a Makepad live_design string and calling framework reload
     b) Interpreting a limited expression subset in CPU and passing result as uniforms
     c) Treating the custom src as a template and swapping a `custom_mode: float` uniform
        that activates a pre-compiled "passthrough" branch in the shader
   - Whichever approach compiles and demonstrates visual change when text is edited is acceptable.
   - Set `self.shader_error = None` on success, `Some(err_msg)` on parse/compile error.
   - Call `self.redraw(cx)`.
   - Return `Ok(())` or `Err(message)`.

**Part B — ShaderEditor widget:**

Create `src/bin/gui/shader_editor.rs`:

```
live_design! {
    pub ShaderEditor = {{ShaderEditor}} {
        width: Fill, height: 180
        show_bg: true
        draw_bg: { color: #11111b }
        flow: Down
        // Header row: label + Apply button
        header = <View> {
            flow: Right, height: 32, padding: 8
            <Label> { text: "Shader Editor" draw_text: { color: #585b70 } }
            apply_btn = <Button> { text: "Apply" ... }
        }
        // Text input area for shader source
        shader_input = <TextInput> {
            width: Fill, height: 130
            draw_text: { text_style: { font_size: 11.0 font: mono } color: #cdd6f4 }
        }
        // Error display label (hidden when no error)
        error_label = <Label> {
            width: Fill
            draw_text: { color: #f38ba8 text_style: { font_size: 10.0 } }
            text: ""
        }
    }
}
```

Struct fields:
- `draw_bg: DrawColor`, `walk: Walk`, `layout: Layout`
- `#[rust] current_src: String`

Methods:
- `pub fn set_source(&mut self, cx: &mut Cx, src: &str)` — sets TextInput text
- `pub fn show_error(&mut self, cx: &mut Cx, err: &str)` — sets error_label text
- `pub fn clear_error(&mut self, cx: &mut Cx)` — clears error_label

Action enum: `pub enum ShaderEditorAction { Apply(String) }`

In `handle_event`: when Apply button clicked, dispatch:
```rust
cx.widget_action(uid, &scope.path, ShaderEditorAction::Apply(
    self.ui.text_input(id!(shader_input)).get_text()
));
```
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo check --bin hum-gui 2>&1 | tail -20</automated>
  </verify>
  <done>
`cargo check` passes. shader_editor.rs exists with ShaderEditor widget. visualizer.rs has reload_shader() and VisualizerPreset::Custom.
  </done>
</task>

<task type="auto">
  <name>Task 2: Wire ShaderEditor into app.rs with toggle</name>
  <files>src/bin/gui/app.rs</files>
  <action>
Modify `src/bin/gui/app.rs`:

1. **Module**: add `mod shader_editor;` and `use crate::shader_editor::ShaderEditorAction;`

2. **live_design! layout** — add shader editor below the visualizer (starts collapsed):
   ```
   body = <View> { flow: Down
     visualizer = <VisualizerView> { height: 220 }
     shader_editor_wrap = <View> {
         width: Fill, height: 0   // collapsed by default
         shader_ed = <ShaderEditor> {}
     }
     mid_zone = <View> { ... }
     transport_bar = <View> {
         // existing buttons...
         shader_toggle = <Button> { text: "SHADER" draw_text: { ... color: (SUBTLE) } }
         // preset buttons...
     }
   }
   ```

3. **App struct**: add `#[rust] shader_editor_open: bool` field (default false).

4. **handle_actions** — wire ShaderEditorAction and toggle:
   ```rust
   // Toggle shader editor pane
   if self.ui.button(id!(shader_toggle)).clicked(actions) {
       self.shader_editor_open = !self.shader_editor_open;
       let h = if self.shader_editor_open { 180.0 } else { 0.0 };
       self.ui.view(id!(shader_editor_wrap)).apply_over(cx, live!{ height: (h) });
       self.ui.redraw(cx);
   }

   // Apply shader edit
   for action in actions {
       if let ShaderEditorAction::Apply(src) = action.cast() {
           let result = self.ui.visualizer(id!(visualizer))
               .borrow_mut()
               .map(|mut v| v.reload_shader(cx, &src));
           match result {
               Some(Ok(())) => {
                   self.ui.shader_editor(id!(shader_ed)).clear_error(cx);
               }
               Some(Err(msg)) => {
                   self.ui.shader_editor(id!(shader_ed)).show_error(cx, &msg);
               }
               None => {}
           }
       }
   }
   ```

5. When a preset button is clicked, also populate the shader editor with the preset's
   default pixel() source string via `set_source()`, so users see the starting point.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -30</automated>
  </verify>
  <done>
`cargo build --bin hum-gui` succeeds. SHADER button in transport bar toggles the editor pane. Editing text and pressing Apply changes the visualization. Compile errors appear in red in the editor pane.
  </done>
</task>

</tasks>

<verification>
- `cargo build --bin hum-gui` passes
- SHADER toggle button shows/hides the editor pane
- Default shader source pre-populated when pane opens
- Apply button triggers reload_shader — visualization changes or error appears
- Old shader keeps running on error (no crash)
</verification>

<success_criteria>
1. Build passes
2. Shader editor pane is toggleable
3. Editing and applying shader source updates the visualization without restart
4. Errors surface inline without crashing the visualizer
</success_criteria>

<output>
After completion, create `.planning/phases/14-milkdrop-viz/14-2-SUMMARY.md`
</output>
