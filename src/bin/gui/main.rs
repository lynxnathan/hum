// Entry point for hum-gui -- Makepad requires app logic in a pub fn app_main()
// called from main, not directly in main(), due to its event loop architecture.
mod app;
mod arrangement_view;
mod beat_detector;
mod shader_editor;
mod spectral_view;
mod transport_client;
mod visualizer;
mod key_handler;
mod terminal_pane;
mod vu_meters;
mod layout_config;
mod project_browser;

pub fn main() {
    app::app_main();
}
