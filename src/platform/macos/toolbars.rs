use cacao::{
    appkit::toolbar::{ItemIdentifier, Toolbar, ToolbarDelegate, ToolbarItem},
    image::{Image, MacSystemIcon, SFSymbol},
};

#[derive(Debug)]
pub struct PreferencesToolbar((ToolbarItem, ToolbarItem));

impl Default for PreferencesToolbar {
    fn default() -> Self {
        PreferencesToolbar((
            {
                let mut item = ToolbarItem::new("add_component");
                item.set_title("Component");

                let icon = Image::symbol(SFSymbol::Custom("plus.app"), "Add Component");
                item.set_image(icon);

                item.set_action(|_| {
                    println!("add component");
                });

                item
            },
            {
                let mut item = ToolbarItem::new("advanced");
                item.set_title(" Component");

                let icon = Image::toolbar_icon(MacSystemIcon::PreferencesAdvanced, "Advanced");
                item.set_image(icon);

                item.set_action(|_| {});

                item
            },
        ))
    }
}

impl ToolbarDelegate for PreferencesToolbar {
    const NAME: &'static str = "PreferencesToolbar";

    fn allowed_item_identifiers(&self) -> Vec<ItemIdentifier> {
        vec![
            ItemIdentifier::Custom("add_component"),
            ItemIdentifier::FlexibleSpace,
            ItemIdentifier::Custom("advanced"),
        ]
    }

    fn default_item_identifiers(&self) -> Vec<ItemIdentifier> {
        vec![
            ItemIdentifier::Custom("add_component"),
            ItemIdentifier::FlexibleSpace,
            ItemIdentifier::Custom("advanced"),
        ]
    }

    fn item_for(&self, identifier: &str) -> &ToolbarItem {
        match identifier {
            "add_component" => &self.0 .0,
            "advanced" => &self.0 .1,
            _ => {
                unreachable!();
            }
        }
    }
}
