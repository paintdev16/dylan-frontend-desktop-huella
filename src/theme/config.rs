use std::rc::Rc;

use gpui_kit::{
    component::{ThemeConfig, ThemeConfigColors, ThemeMode},
    *,
};

fn color(value: &'static str) -> Option<SharedString> {
    Some(SharedString::from(value))
}

pub fn dylan_light_config() -> Rc<ThemeConfig> {
    let mut colors = ThemeConfigColors::default();
    colors.background = color("#F3F4FF");
    colors.foreground = color("#0F1430");
    colors.border = color("#E3E1F7");
    colors.input = color("#CFD2EE");
    colors.ring = color("#6438E8");
    colors.caret = color("#6438E8");
    colors.selection = color("#6438E833");
    colors.drag_border = color("#6438E8");
    colors.drop_target = color("#6438E826");
    colors.overlay = color("#0F143024");
    colors.window_border = color("#E3E1F7");

    colors.primary = color("#6438E8");
    colors.primary_hover = color("#542BD1");
    colors.primary_active = color("#4622B5");
    colors.primary_foreground = color("#FFFFFF");
    colors.button_primary = color("#6438E8");
    colors.button_primary_hover = color("#542BD1");
    colors.button_primary_active = color("#4622B5");
    colors.button_primary_foreground = color("#FFFFFF");

    colors.button = color("#FFFFFF");
    colors.button_hover = color("#F0EEFF");
    colors.button_active = color("#E4E0FC");
    colors.button_foreground = color("#191F3D");

    colors.secondary = color("#FFFFFF");
    colors.secondary_hover = color("#F0EEFF");
    colors.secondary_active = color("#E4E0FC");
    colors.secondary_foreground = color("#191F3D");
    colors.button_secondary = color("#FFFFFF");
    colors.button_secondary_hover = color("#F0EEFF");
    colors.button_secondary_active = color("#E4E0FC");
    colors.button_secondary_foreground = color("#191F3D");

    colors.accent = color("#E3DAFC");
    colors.accent_foreground = color("#552BD4");
    colors.muted = color("#ECEEFF");
    colors.muted_foreground = color("#5A6282");

    colors.danger = color("#D92D20");
    colors.danger_hover = color("#B42318");
    colors.danger_active = color("#912018");
    colors.danger_foreground = color("#FFFFFF");
    colors.button_danger = color("#D92D20");
    colors.button_danger_hover = color("#B42318");
    colors.button_danger_active = color("#912018");
    colors.button_danger_foreground = color("#FFFFFF");

    colors.success = color("#15803D");
    colors.success_hover = color("#166534");
    colors.success_active = color("#14532D");
    colors.success_foreground = color("#FFFFFF");
    colors.button_success = color("#15803D");
    colors.button_success_hover = color("#166534");
    colors.button_success_active = color("#14532D");
    colors.button_success_foreground = color("#FFFFFF");

    colors.warning = color("#B45309");
    colors.warning_hover = color("#92400E");
    colors.warning_active = color("#78350F");
    colors.warning_foreground = color("#FFFFFF");
    colors.button_warning = color("#B45309");
    colors.button_warning_hover = color("#92400E");
    colors.button_warning_active = color("#78350F");
    colors.button_warning_foreground = color("#FFFFFF");

    colors.info = color("#2563EB");
    colors.info_hover = color("#1D4ED8");
    colors.info_active = color("#1E40AF");
    colors.info_foreground = color("#FFFFFF");
    colors.button_info = color("#2563EB");
    colors.button_info_hover = color("#1D4ED8");
    colors.button_info_active = color("#1E40AF");
    colors.button_info_foreground = color("#FFFFFF");

    colors.popover = color("#FFFFFF");
    colors.popover_foreground = color("#0F1430");
    colors.group_box = color("#FFFFFF");
    colors.group_box_foreground = color("#0F1430");
    colors.accordion = color("#FFFFFF");
    colors.skeleton = color("#E6E4FA");

    colors.title_bar = color("#FFFFFF");
    colors.title_bar_border = color("#E3E1F7");
    colors.status_bar = color("#FFFFFF");
    colors.status_bar_border = color("#E3E1F7");

    colors.sidebar = color("#FFFFFF");
    colors.sidebar_foreground = color("#191F3D");
    colors.sidebar_border = color("#E3E1F7");
    colors.sidebar_accent = color("#E3DAFC");
    colors.sidebar_accent_foreground = color("#552BD4");
    colors.sidebar_primary = color("#6438E8");
    colors.sidebar_primary_foreground = color("#FFFFFF");

    colors.list = color("#FFFFFF");
    colors.list_even = color("#F8F8FF");
    colors.list_head = color("#F2F1FE");
    colors.list_hover = color("#F2F1FE");
    colors.list_active = color("#6438E81A");
    colors.list_active_border = color("#6438E8");
    colors.list_highlight_foreground = color("#6438E8");

    colors.table = color("#FFFFFF");
    colors.table_even = color("#F8F8FF");
    colors.table_head = color("#F2F1FE");
    colors.table_head_foreground = color("#5A6282");
    colors.table_foot = color("#F2F1FE");
    colors.table_foot_foreground = color("#0F1430");
    colors.table_hover = color("#F2F1FE");
    colors.table_active = color("#6438E81A");
    colors.table_active_border = color("#6438E8");
    colors.table_row_border = color("#E6E4F8");

    colors.tab = color("#00000000");
    colors.tab_foreground = color("#5A6282");
    colors.tab_active = color("#FFFFFF");
    colors.tab_active_foreground = color("#0F1430");
    colors.tab_bar = color("#ECEEFF");
    colors.tab_bar_segmented = color("#ECEEFF");

    colors.link = color("#6438E8");
    colors.link_hover = color("#542BD1");
    colors.link_active = color("#4622B5");

    colors.progress_bar = color("#6438E8");
    colors.slider_bar = color("#6438E8");
    colors.slider_thumb = color("#FFFFFF");
    colors.switch = color("#CFD2EE");
    colors.switch_thumb = color("#FFFFFF");

    colors.scrollbar = color("#F3F4FF00");
    colors.scrollbar_thumb = color("#9C9EC4CC");
    colors.scrollbar_thumb_hover = color("#7A7EAF");

    let mut config = ThemeConfig::default();
    config.is_default = true;
    config.name = SharedString::from("Comedor Dylan Claro");
    config.mode = ThemeMode::Light;
    config.font_size = Some(14.0);
    config.radius = Some(8);
    config.radius_lg = Some(12);
    config.shadow = Some(true);
    config.colors = colors;
    Rc::new(config)
}

pub fn dylan_dark_config() -> Rc<ThemeConfig> {
    let mut colors = ThemeConfigColors::default();
    colors.background = color("#11152E");
    colors.foreground = color("#F4F6FF");
    colors.border = color("#2C3566");
    colors.input = color("#3B4785");
    colors.ring = color("#A78BFA");
    colors.caret = color("#A78BFA");
    colors.selection = color("#A78BFA40");
    colors.drag_border = color("#A78BFA");
    colors.drop_target = color("#A78BFA33");
    colors.overlay = color("#07091780");
    colors.window_border = color("#2C3566");

    colors.primary = color("#A78BFA");
    colors.primary_hover = color("#B8A1FC");
    colors.primary_active = color("#9070F5");
    colors.primary_foreground = color("#11152E");
    colors.button_primary = color("#6E48F0");
    colors.button_primary_hover = color("#7D5AF5");
    colors.button_primary_active = color("#5B37DA");
    colors.button_primary_foreground = color("#FFFFFF");

    colors.button = color("#1E254D");
    colors.button_hover = color("#283163");
    colors.button_active = color("#313C78");
    colors.button_foreground = color("#F4F6FF");

    colors.secondary = color("#1E254D");
    colors.secondary_hover = color("#283163");
    colors.secondary_active = color("#313C78");
    colors.secondary_foreground = color("#F4F6FF");
    colors.button_secondary = color("#1E254D");
    colors.button_secondary_hover = color("#283163");
    colors.button_secondary_active = color("#313C78");
    colors.button_secondary_foreground = color("#F4F6FF");

    colors.accent = color("#2B2F63");
    colors.accent_foreground = color("#DDD6FE");
    colors.muted = color("#222952");
    colors.muted_foreground = color("#A9B4D4");

    colors.danger = color("#D92D20");
    colors.danger_hover = color("#E04035");
    colors.danger_active = color("#B42318");
    colors.danger_foreground = color("#FFFFFF");
    colors.button_danger = color("#D92D20");
    colors.button_danger_hover = color("#E04035");
    colors.button_danger_active = color("#B42318");
    colors.button_danger_foreground = color("#FFFFFF");

    colors.success = color("#15803D");
    colors.success_hover = color("#16A34A");
    colors.success_active = color("#166534");
    colors.success_foreground = color("#FFFFFF");
    colors.button_success = color("#15803D");
    colors.button_success_hover = color("#16A34A");
    colors.button_success_active = color("#166534");
    colors.button_success_foreground = color("#FFFFFF");

    colors.warning = color("#B45309");
    colors.warning_hover = color("#D97706");
    colors.warning_active = color("#92400E");
    colors.warning_foreground = color("#FFFFFF");
    colors.button_warning = color("#B45309");
    colors.button_warning_hover = color("#D97706");
    colors.button_warning_active = color("#92400E");
    colors.button_warning_foreground = color("#FFFFFF");

    colors.info = color("#2563EB");
    colors.info_hover = color("#3B82F6");
    colors.info_active = color("#1D4ED8");
    colors.info_foreground = color("#FFFFFF");
    colors.button_info = color("#2563EB");
    colors.button_info_hover = color("#3B82F6");
    colors.button_info_active = color("#1D4ED8");
    colors.button_info_foreground = color("#FFFFFF");

    colors.popover = color("#181E3E");
    colors.popover_foreground = color("#F4F6FF");
    colors.group_box = color("#181E3E");
    colors.group_box_foreground = color("#F4F6FF");
    colors.accordion = color("#181E3E");
    colors.skeleton = color("#222A54");

    colors.title_bar = color("#141936");
    colors.title_bar_border = color("#2C3566");
    colors.status_bar = color("#141936");
    colors.status_bar_border = color("#2C3566");

    colors.sidebar = color("#141936");
    colors.sidebar_foreground = color("#F4F6FF");
    colors.sidebar_border = color("#2C3566");
    colors.sidebar_accent = color("#2B2F63");
    colors.sidebar_accent_foreground = color("#DDD6FE");
    colors.sidebar_primary = color("#A78BFA");
    colors.sidebar_primary_foreground = color("#11152E");

    colors.list = color("#181E3E");
    colors.list_even = color("#1E254D");
    colors.list_head = color("#151A38");
    colors.list_hover = color("#242C59");
    colors.list_active = color("#A78BFA26");
    colors.list_active_border = color("#A78BFA");
    colors.list_highlight_foreground = color("#A78BFA");

    colors.table = color("#181E3E");
    colors.table_even = color("#1E254D");
    colors.table_head = color("#151A38");
    colors.table_head_foreground = color("#A9B4D4");
    colors.table_foot = color("#151A38");
    colors.table_foot_foreground = color("#F4F6FF");
    colors.table_hover = color("#242C59");
    colors.table_active = color("#A78BFA26");
    colors.table_active_border = color("#A78BFA");
    colors.table_row_border = color("#2A3361");

    colors.tab = color("#00000000");
    colors.tab_foreground = color("#A9B4D4");
    colors.tab_active = color("#181E3E");
    colors.tab_active_foreground = color("#F4F6FF");
    colors.tab_bar = color("#141936");
    colors.tab_bar_segmented = color("#141936");

    colors.link = color("#A78BFA");
    colors.link_hover = color("#C4B5FD");
    colors.link_active = color("#8B5CF6");

    colors.progress_bar = color("#A78BFA");
    colors.slider_bar = color("#A78BFA");
    colors.slider_thumb = color("#11152E");
    colors.switch = color("#3B4785");
    colors.switch_thumb = color("#F4F6FF");

    colors.scrollbar = color("#11152E00");
    colors.scrollbar_thumb = color("#55629ECC");
    colors.scrollbar_thumb_hover = color("#7584C7");

    let mut config = ThemeConfig::default();
    config.is_default = true;
    config.name = SharedString::from("Comedor Dylan Oscuro");
    config.mode = ThemeMode::Dark;
    config.font_size = Some(14.0);
    config.radius = Some(8);
    config.radius_lg = Some(12);
    config.shadow = Some(true);
    config.colors = colors;
    Rc::new(config)
}
