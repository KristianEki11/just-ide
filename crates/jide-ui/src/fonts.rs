use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::collections::BTreeMap;

const INTER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
const JETBRAINS_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_BOLD: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
const JETBRAINS_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf");

/// Neutralizes `calt`, `liga`, and `clig` OpenType feature tags in `GSUB` `FeatureList`
/// so code sequences like `!=`, `->`, `=>`, `<=` always render their exact ASCII characters.
#[must_use]
pub fn disable_ligatures_in_ttf(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    if out.len() < 12 {
        return out;
    }
    let num_tables = usize::from(u16::from_be_bytes([out[4], out[5]]));
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        if rec + 16 > out.len() {
            break;
        }
        if &out[rec..rec + 4] == b"GSUB" {
            let offset =
                u32::from_be_bytes([out[rec + 8], out[rec + 9], out[rec + 10], out[rec + 11]])
                    as usize;
            let length =
                u32::from_be_bytes([out[rec + 12], out[rec + 13], out[rec + 14], out[rec + 15]])
                    as usize;
            if let Some(end) = offset.checked_add(length) {
                if end <= out.len() && offset + 8 <= out.len() {
                    let feat_rel =
                        usize::from(u16::from_be_bytes([out[offset + 6], out[offset + 7]]));
                    let feat_abs = offset + feat_rel;
                    if feat_abs + 2 <= end {
                        let count =
                            usize::from(u16::from_be_bytes([out[feat_abs], out[feat_abs + 1]]));
                        for f in 0..count {
                            let tag_off = feat_abs + 2 + f * 6;
                            if tag_off + 6 > end {
                                break;
                            }
                            let tag = &mut out[tag_off..tag_off + 4];
                            if tag == b"calt" || tag == b"liga" || tag == b"clig" {
                                tag.copy_from_slice(b"xxxx");
                            }
                        }
                    }
                }
            }
            break;
        }
    }
    out
}

fn jetbrains_font_data(bytes: &'static [u8], ligatures: bool) -> FontData {
    if ligatures {
        FontData::from_static(bytes)
    } else {
        FontData::from_owned(disable_ligatures_in_ttf(bytes))
    }
}

fn prepend_unique(list: &mut Vec<String>, name: &str) {
    list.retain(|item| item != name);
    list.insert(0, name.to_owned());
}

fn append_before_egui_emoji(list: &mut Vec<String>, name: &str) {
    if list.iter().any(|item| item == name) {
        return;
    }
    let pos = list
        .iter()
        .position(|item| {
            item == "egui-icons" || item == "NotoEmoji-Regular" || item == "emoji-icon-font"
        })
        .unwrap_or(list.len());
    list.insert(pos, name.to_owned());
}

fn initial_font_definitions(ligatures: bool) -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    defs.font_data.insert(
        "Inter-Regular".to_owned(),
        FontData::from_static(INTER_REGULAR).into(),
    );
    defs.font_data.insert(
        "JetBrainsMono-Regular".to_owned(),
        jetbrains_font_data(JETBRAINS_REGULAR, ligatures).into(),
    );

    let prop = defs.families.entry(FontFamily::Proportional).or_default();
    prepend_unique(prop, "JetBrainsMono-Regular");
    prepend_unique(prop, "Inter-Regular");

    let mono = defs.families.entry(FontFamily::Monospace).or_default();
    prepend_unique(mono, "Inter-Regular");
    prepend_unique(mono, "JetBrainsMono-Regular");

    #[cfg(target_os = "windows")]
    for (name, path) in [
        ("Fallback-SegoeSymbol", r"C:\Windows\Fonts\seguisym.ttf"),
        ("Fallback-SegoeUI", r"C:\Windows\Fonts\segoeui.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            defs.font_data
                .insert(name.to_owned(), FontData::from_owned(bytes).into());
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                let list = defs.families.entry(family).or_default();
                append_before_egui_emoji(list, name);
            }
        }
    }

    defs
}

/// Registers the primary embedded UI and editor font weights before Frame 0.
pub fn configure_initial_fonts(ctx: &egui::Context) {
    configure_fonts(ctx, false);
}

/// Registers UI and editor fonts with optional programming ligatures.
pub fn configure_fonts(ctx: &egui::Context, ligatures: bool) {
    ctx.set_fonts(initial_font_definitions(ligatures));

    ctx.all_styles_mut(|style| {
        style.text_styles = text_styles();
        style.spacing.item_spacing = egui::vec2(8.0, 4.0);
        style.spacing.button_padding = egui::vec2(12.0, 4.0);
        style.spacing.interact_size = egui::vec2(28.0, 28.0);
    });
}

/// Spawns a background thread to register secondary weights and Windows Unicode/CJK fallbacks cleanly.
pub fn spawn_lazy_font_loader(ctx: &egui::Context, ligatures: bool) {
    let ctx = ctx.clone();
    let _ = std::thread::Builder::new()
        .name("jide-font-loader".to_owned())
        .spawn(move || {
            let mut defs = initial_font_definitions(ligatures);
            for (name, bytes, family, is_mono) in [
                (
                    "Inter-Medium",
                    INTER_MEDIUM,
                    FontFamily::Proportional,
                    false,
                ),
                (
                    "Inter-SemiBold",
                    INTER_SEMIBOLD,
                    FontFamily::Proportional,
                    false,
                ),
                (
                    "JetBrainsMono-Bold",
                    JETBRAINS_BOLD,
                    FontFamily::Monospace,
                    true,
                ),
                (
                    "JetBrainsMono-Italic",
                    JETBRAINS_ITALIC,
                    FontFamily::Monospace,
                    true,
                ),
            ] {
                let data = if is_mono {
                    jetbrains_font_data(bytes, ligatures)
                } else {
                    FontData::from_static(bytes)
                };
                defs.font_data.insert(name.to_owned(), data.into());
                let list = defs.families.entry(family).or_default();
                append_before_egui_emoji(list, name);
            }

            #[cfg(target_os = "windows")]
            for (name, path) in [
                ("Fallback-Consolas", r"C:\Windows\Fonts\consola.ttf"),
                ("Fallback-SegoeEmoji", r"C:\Windows\Fonts\seguiemj.ttf"),
                ("Fallback-Malgun", r"C:\Windows\Fonts\malgun.ttf"),
            ] {
                if let Ok(bytes) = std::fs::read(path) {
                    defs.font_data
                        .insert(name.to_owned(), FontData::from_owned(bytes).into());
                    for family in [FontFamily::Proportional, FontFamily::Monospace] {
                        let list = defs.families.entry(family).or_default();
                        append_before_egui_emoji(list, name);
                    }
                }
            }

            ctx.set_fonts(defs);
        });
}

fn text_styles() -> BTreeMap<TextStyle, FontId> {
    [
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(18.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(14.0, FontFamily::Monospace),
        ),
    ]
    .into()
}
