use super::*;

#[test]
fn resizing_renderer_layers_preserves_pixel_scale_and_leaves_siblings_unchanged() {
    let parent = CALayer::new();
    let sibling = CALayer::new();
    let content = CALayer::new();
    let drawable = CALayer::new();
    parent.addSublayer(&sibling);
    parent.addSublayer(&content);
    content.addSublayer(&drawable);
    drawable.setContentsScale(2.);
    let mut frame = drawable.frame();
    frame.size.width = 1440.;
    frame.size.height = 920.;
    drawable.setFrame(frame);
    assert_eq!(&*drawable.contentsGravity(), ns_string!("resize"));

    anchor_renderer_layers(&content);
    for (width, height) in [(1080., 720.), (1440., 920.), (1800., 1000.), (1080., 720.)] {
        frame.size.width = width;
        frame.size.height = height;
        drawable.setFrame(frame);
        assert_eq!(&*drawable.contentsGravity(), ns_string!("topLeft"));
        assert_eq!(drawable.contentsScale(), 2.);
        assert_eq!(drawable.frame().size, frame.size);
    }
    assert_eq!(&*sibling.contentsGravity(), ns_string!("resize"));
    assert_eq!(child_layers(&parent), vec![sibling, content]);
}

#[test]
fn anchoring_does_not_replace_existing_layer_geometry_or_scaling() {
    let content = CALayer::new();
    let drawable = CALayer::new();
    content.addSublayer(&drawable);
    drawable.setGeometryFlipped(true);
    drawable.setContentsScale(1.5);
    let geometry = drawable.isGeometryFlipped();
    let flipped = drawable.contentsAreFlipped();
    anchor_renderer_layers(&content);
    let gravity = drawable.contentsGravity();
    assert_eq!(
        &*gravity,
        if flipped {
            ns_string!("bottomLeft")
        } else {
            ns_string!("topLeft")
        }
    );
    assert_eq!(drawable.isGeometryFlipped(), geometry);
    assert_eq!(drawable.contentsScale(), 1.5);
    anchor_renderer_layers(&content);
    assert_eq!(drawable.contentsGravity(), gravity);
}

#[test]
fn zoom_restores_each_window_only_while_its_frame_is_unchanged() {
    use objc2_foundation::{NSPoint, NSSize};

    let first = iced::window::Id::unique();
    let second = iced::window::Id::unique();
    let original = NSRect::new(NSPoint::new(100., 80.), NSSize::new(1080., 720.));
    let maximized = NSRect::new(NSPoint::new(0., 25.), NSSize::new(1600., 950.));
    let resized = NSRect::new(maximized.origin, NSSize::new(1400., 920.));
    let mut zoom = Zoom::default();
    zoom.frames.insert(first, (original, maximized));
    zoom.frames.insert(second, (resized, maximized));

    assert_eq!(zoom.restore_frame(first, resized), None);
    assert_eq!(zoom.restore_frame(first, maximized), None);
    assert_eq!(zoom.restore_frame(second, maximized), Some(resized));
    assert_eq!(zoom.restore_frame(second, maximized), None);
}

#[test]
fn zoom_interpolates_position_and_size_with_eased_endpoints() {
    use objc2_foundation::{NSPoint, NSSize};

    let original = NSRect::new(NSPoint::new(100., 80.), NSSize::new(1080., 720.));
    let maximized = NSRect::new(NSPoint::new(0., 24.), NSSize::new(1600., 960.));
    for (from, to) in [(original, maximized), (maximized, original)] {
        assert_eq!(interpolate(from, to, 0.), from);
        assert_eq!(interpolate(from, to, 1.), to);
        let midpoint = interpolate(from, to, 0.5);
        assert_eq!(midpoint.origin, NSPoint::new(50., 52.));
        assert_eq!(midpoint.size, NSSize::new(1340., 840.));
        assert!(
            (interpolate(from, to, 0.25).size.width - from.size.width).abs()
                < (to.size.width - from.size.width).abs() * 0.25
        );
    }
}
