pub mod toolbars;

use cacao::appkit::{toolbar::Toolbar, window::WindowToolbarStyle};
use objc::{msg_send, runtime::Object, sel, sel_impl};
use objc_id::ShareId;
use platform::toolbars::PreferencesToolbar;
use winit::platform::macos::{WindowBuilderExtMacOS, WindowExtMacOS};

pub fn create_toolbar() {
    let tb = Toolbar::new("PreferencesToolbar", PreferencesToolbar::default());
    tb.set_display_mode(cacao::appkit::toolbar::ToolbarDisplayMode::IconAndLabel);

    let win = unsafe {
        cacao::appkit::window::Window::<()> {
            objc: ShareId::from_ptr(window.ns_window() as *mut Object),
            delegate: None,
        }
    };
    win.set_title_visibility(cacao::appkit::window::TitleVisibility::Hidden);
    win.set_toolbar(&tb);
    // win.styl
    unsafe {
        // sel_impl!()
        let _: () = msg_send![&*(window.ns_window() as *mut Object), setToolbarStyle:WindowToolbarStyle::Unified];
    }
}
