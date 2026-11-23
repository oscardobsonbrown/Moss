use super::{editor, theme::Theme};
use crate::state::State;
use moss_core::Block;
use rune::*;

pub(super) fn view(
    s: &State,
    block: &Block,
    index: usize,
    t: Theme,
    available_width: f32,
    zones: &std::rc::Rc<Vec<crate::editor_drag::DropZone>>,
) -> El {
    row()
        .key(format!("block-{}", block.id))
        .w_full()
        .min_w(0.)
        .gap(6.)
        .child(editor::gutter(s, &block.id, index, false, 26., t, zones))
        .child(content(s, block, t, available_width))
}
pub(super) fn content(s: &State, block: &Block, t: Theme, available_width: f32) -> El {
    let asset = block
        .config
        .get("asset")
        .and_then(serde_json::Value::as_str);
    let image = asset.and_then(|id| s.page_cover.images.get(id));
    let max_width = rune::memo::window_layout_with(&format!("block-{}", block.id), |rect| rect[2])
        .unwrap_or_else(|| (available_width.min(860.) - 140.).max(120.))
        .max(1.);
    let page = block.page_id.clone();
    let id = block.id.clone();
    let expected = asset.map(str::to_owned);
    let pick_image = move |button: El| {
        button.on_press_tx(move |_: &mut State, tx| {
            let (page, id, expected) = (page.clone(), id.clone(), expected.clone());
            tx.open_file(
                FileDialogOptions {
                    title: "Choose an image".into(),
                    ..Default::default()
                }
                .filter("Images", &["png", "jpg", "jpeg", "webp"]),
                move |s: &mut State, result, _| match result {
                    Ok(Some(path)) => s.upload_block_image(&page, &id, expected.clone(), path),
                    Ok(None) => {}
                    Err(error) => s.error = Some(format!("Image is not selected: {error}")),
                },
            );
        })
    };
    let key = format!("image-upload-{}", block.id);
    let mut content = col().grow(1.).min_w(0.);
    if let Some(image) = image {
        if let Some([w, h]) = image.size() {
            let width = (w as f32).min(max_width).max(1.);
            let height = (h as f32 * width / w.max(1) as f32).max(1.);
            let replace = pick_image(
                super::theme::button(key, "Replace", t)
                    .label("Replace image")
                    .h(26.)
                    .px(9.)
                    .radius(6.)
                    .fill(t.menu.alpha(0.92))
                    .border(1., t.line)
                    .shadow(1., 4., Color::BLACK.alpha(0.18)),
            );
            return content.child(
                el().key(format!("image-frame-{}", block.id))
                    .size(width, height)
                    .hover_group()
                    .child(
                        el().key(format!("image-content-{}", block.id))
                            .size(width, height)
                            .pixels(image.pixels()),
                    )
                    .child(
                        replace
                            .pin(None, Some(8.), Some(8.), None)
                            .group_opacity(if s.sidebar_keyboard_controls { 1. } else { 0. }, 1.),
                    ),
            );
        } else {
            content = content.child(t.label("Loading image…", t.muted));
        }
    } else if let Some(error) = asset.and_then(|id| s.page_cover.image_errors.get(id)) {
        content = content.child(
            t.label(format!("Image unavailable: {error}"), t.error)
                .wrap(),
        );
    }
    content.child(pick_image(super::theme::button(
        key,
        if asset.is_some() {
            "Replace image"
        } else {
            "Upload an image"
        },
        t,
    )))
}
