use super::*;
use iced::advanced::text::Highlighter as _;

#[tokio::test]
async fn renderer_requests_visible_blocks_and_preserves_multiline_state_after_edits() {
    use iced::futures::StreamExt;
    let cache = Cache::default();
    let text = Arc::new(format!(
        "{}/* open\r\n{}end */",
        "let a = 1;\r\n".repeat(BLOCK - 1),
        "continued\r\n".repeat(BLOCK)
    ));
    let mut settings = cache.settings(&text, "rs".into());
    let mut renderer = Highlighter::new(&settings);
    assert_eq!(renderer.highlight_line("let a = 1;").count(), 0);
    let mut stream = Box::pin(worker::stream(&(7, settings.clone())));
    let (_, first) = stream.next().await.unwrap();
    assert_eq!(first.blocks.len(), 1);
    settings.cache.prepared = Some(first.clone());
    renderer.update(&settings);
    assert!(renderer.highlight_line("let a = 1;").count() > 0);
    for _ in 1..=BLOCK {
        renderer.highlight_line("").count();
    }
    let (_, second) = stream.next().await.unwrap();
    assert_eq!(second.blocks.len(), 2);
    let mut retained = Cache::default();
    let path = Some(std::path::Path::new("a.rs"));
    retained.accept(second.clone(), &text, path);
    retained.accept(first.clone(), &text, path);
    assert!(Arc::ptr_eq(retained.prepared.as_ref().unwrap(), &second));
    assert_eq!(
        second.blocks[1].lines[0],
        vec![(0..9, crate::ui::theme::syntax::COMMENT)]
    );
    let changed = Arc::new(text.replace("continued", "different"));
    let cache = Cache {
        prepared: Some(second.clone()),
        ..cache
    };
    assert_eq!(cache.settings(&changed, "rs".into()).reusable(), 1);
    let edited = Arc::new(text.replace("/* open", "let b = 1;"));
    let settings = cache.settings(&edited, "rs".into());
    assert_eq!(settings.reusable(), 0);
    let mut pending = Cache::default();
    pending.accept(second.clone(), &edited, path);
    pending.accept(second.clone(), &text, Some(std::path::Path::new("a.md")));
    assert!(pending.prepared.is_none());
    renderer.update(&settings);
    assert_eq!(renderer.current_line(), 0);
    assert_eq!(renderer.highlight_line("").count(), 0);
    let mut edited_stream = Box::pin(worker::stream(&(7, settings)));
    let (_, recolored) = edited_stream.next().await.unwrap();
    assert_ne!(
        recolored.blocks[0].lines[BLOCK - 1],
        first.blocks[0].lines[BLOCK - 1]
    );
}
