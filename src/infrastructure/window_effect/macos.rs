//! Configure native macOS window effects.

use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSApplication, NSView,
    NSViewLayerContentsPlacement, NSViewLayerContentsRedrawPolicy, NSWindow, NSWindowButton,
    NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{NSArray, NSObjectNSKeyValueCoding, NSPoint, NSRect, ns_string};
use objc2_quartz_core::CALayer;
use raw_window_handle::RawWindowHandle;
use std::time::{Duration, Instant};

const ZOOM_DURATION: Duration = Duration::from_millis(240);
pub(crate) fn fade_for_exit(
    window: &dyn iced::window::Window,
    finished: iced::futures::channel::oneshot::Sender<()>,
) {
    let Some((window, _)) = native_window(window) else {
        return;
    };
    if !window.isVisible()
        || NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    {
        return;
    }

    let finished = std::cell::Cell::new(Some(finished));
    let completion = block2::RcBlock::new(move || {
        if let Some(sender) = finished.take() {
            let _ = sender.send(());
        }
    });
    NSAnimationContext::beginGrouping();
    let context = NSAnimationContext::currentContext();
    context.setDuration(ZOOM_DURATION.as_secs_f64());
    context.setCompletionHandler(Some(&completion));
    window.animator().setAlphaValue(0.);
    NSAnimationContext::endGrouping();
}

struct Transition {
    start: Instant,
    from: NSRect,
    to: NSRect,
    last_frame: NSRect,
}

fn interpolate(from: NSRect, to: NSRect, progress: f64) -> NSRect {
    let eased = progress * progress * (3. - 2. * progress);
    let blend = |from: f64, to: f64| from + (to - from) * eased;
    NSRect::new(
        objc2_foundation::NSPoint::new(
            blend(from.origin.x, to.origin.x),
            blend(from.origin.y, to.origin.y),
        ),
        objc2_foundation::NSSize::new(
            blend(from.size.width, to.size.width),
            blend(from.size.height, to.size.height),
        ),
    )
}

#[derive(Default)]
pub(crate) struct Zoom {
    frames: std::collections::HashMap<iced::window::Id, (NSRect, NSRect)>,
    transitions: std::collections::HashMap<iced::window::Id, Transition>,
}

impl Zoom {
    pub(crate) fn forget(&mut self, id: iced::window::Id) {
        self.frames.remove(&id);
        self.transitions.remove(&id);
    }

    fn restore_frame(&mut self, id: iced::window::Id, current: NSRect) -> Option<NSRect> {
        let (previous, maximized) = self.frames.remove(&id)?;
        (current == maximized).then_some(previous)
    }

    pub(crate) fn toggle(&mut self, id: iced::window::Id, window: &dyn iced::window::Window) {
        let Some((native_window, _)) = native_window(window) else {
            return;
        };
        if native_window
            .styleMask()
            .contains(NSWindowStyleMask::FullScreen)
        {
            return;
        }
        let Some(screen) = native_window.screen() else {
            return;
        };
        let current = native_window.frame();
        let visible = screen.visibleFrame();
        let target = match self.transitions.remove(&id) {
            Some(transition) if transition.last_frame == current => {
                let (original, maximized) = self.frames[&id];
                if transition.to == maximized {
                    original
                } else {
                    maximized
                }
            }
            _ => self.restore_frame(id, current).unwrap_or_else(|| {
                self.frames.insert(id, (current, visible));
                visible
            }),
        };
        if current == target {
            self.forget(id);
        } else if NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
            native_window.setFrame_display(target, true);
            self.finish(id, target, native_window.frame());
        } else {
            self.frames.entry(id).or_insert((target, current));
            self.transitions.insert(
                id,
                Transition {
                    start: Instant::now(),
                    from: current,
                    to: target,
                    last_frame: current,
                },
            );
        }
    }

    pub(crate) fn is_animating(&self) -> bool {
        !self.transitions.is_empty()
    }

    pub(crate) fn active_windows(&self) -> Vec<iced::window::Id> {
        self.transitions.keys().copied().collect()
    }

    pub(crate) fn tick(&mut self, id: iced::window::Id, window: &dyn iced::window::Window) {
        let Some(transition) = self.transitions.get_mut(&id) else {
            return;
        };
        let Some((window, _)) = native_window(window) else {
            self.forget(id);
            return;
        };
        if window.frame() != transition.last_frame
            || window.isMiniaturized()
            || window.styleMask().contains(NSWindowStyleMask::FullScreen)
        {
            self.forget(id);
            return;
        }
        let progress = transition.start.elapsed().as_secs_f64() / ZOOM_DURATION.as_secs_f64();
        let target = transition.to;
        if progress >= 1. {
            window.setFrame_display(target, false);
            self.transitions.remove(&id);
            self.finish(id, target, window.frame());
        } else {
            window.setFrame_display(interpolate(transition.from, target, progress), false);
            transition.last_frame = window.frame();
        }
    }

    fn finish(&mut self, id: iced::window::Id, target: NSRect, actual: NSRect) {
        if let Some((original, maximized)) = self.frames.get_mut(&id) {
            if target == *original {
                self.frames.remove(&id);
            } else {
                *maximized = actual;
            }
        }
    }
}

fn native_window(
    window: &dyn iced::window::Window,
) -> Option<(Retained<NSWindow>, Retained<NSView>)> {
    let mtm = MainThreadMarker::new()?;
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };

    NSApplication::sharedApplication(mtm)
        .windows()
        .to_vec()
        .into_iter()
        .find_map(|native_window| {
            let content = native_window.contentView()?;
            (Retained::as_ptr(&content).cast() == handle.ns_view.as_ptr())
                .then_some((native_window, content))
        })
}

pub(crate) fn configure_resize(window: &dyn iced::window::Window) {
    let Some((_, content)) = native_window(window) else {
        return;
    };

    // Preserve pixel size while the renderer prepares a frame for the new window bounds.
    content.setLayerContentsPlacement(NSViewLayerContentsPlacement::TopLeft);
    content.setLayerContentsRedrawPolicy(NSViewLayerContentsRedrawPolicy::DuringViewResize);
    if let Some(layer) = content.layer() {
        anchor_renderer_layers(&layer);
    }
}

pub(crate) fn align_controls(window: &dyn iced::window::Window, center: f64) {
    let Some((window, content)) = native_window(window) else {
        return;
    };
    if window.styleMask().contains(NSWindowStyleMask::FullScreen) {
        return;
    }
    let buttons = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .map(|kind| window.standardWindowButton(kind));
    let [Some(close), Some(minimize), Some(zoom)] = buttons else {
        return;
    };
    let Some(bar) = parent_view(&close) else {
        return;
    };
    let Some(container) = parent_view(&bar) else {
        return;
    };
    if bar.isFlipped()
        || container.isFlipped()
        || [&minimize, &zoom]
            .iter()
            .any(|button| parent_view(button).is_none_or(|parent| !std::ptr::eq(&*parent, &*bar)))
    {
        return;
    }

    let bounds = content.bounds();
    let y = if content.isFlipped() {
        bounds.origin.y + center
    } else {
        bounds.origin.y + bounds.size.height - center
    };
    let target = content.convertPoint_toView(NSPoint::new(bounds.origin.x, y), None);
    let buttons = [close, minimize, zoom];
    let half_height = buttons
        .iter()
        .map(|button| button.frame().size.height / 2.)
        .fold(0_f64, f64::max);
    let target_y = bar.convertPoint_fromView(target, None).y;
    let extension = (bar.bounds().origin.y + half_height - target_y).max(0.);
    if extension > 0. {
        // Keep the entire button inside the title bar's hit-test area.
        let mut frame = container.frame();
        let mut bar_frame = bar.frame();
        frame.origin.y -= extension;
        frame.size.height += extension;
        bar_frame.size.height += extension;
        container.setFrame(frame);
        bar.setFrame(bar_frame);
    }

    let target_y = bar.convertPoint_fromView(target, None).y;
    for button in buttons {
        let frame = button.frame();
        let y = target_y - frame.size.height / 2.;
        if y != frame.origin.y {
            button.setFrameOrigin(NSPoint::new(frame.origin.x, y));
        }
    }
}

fn parent_view(view: &NSView) -> Option<Retained<NSView>> {
    view.valueForKey(ns_string!("superview"))?.downcast().ok()
}

fn anchor_renderer_layers(layer: &CALayer) {
    // Core Animation's default "resize" gravity stretches the previous drawable between frames.
    layer.setContentsGravity(if layer.contentsAreFlipped() {
        ns_string!("bottomLeft")
    } else {
        ns_string!("topLeft")
    });
    for child in child_layers(layer) {
        anchor_renderer_layers(&child);
    }
}

fn child_layers(layer: &CALayer) -> Vec<Retained<CALayer>> {
    // Check dynamic collection types instead of using the unchecked native sublayers binding.
    layer
        .valueForKey(ns_string!("sublayers"))
        .and_then(|value| value.downcast::<NSArray<AnyObject>>().ok())
        .map(|children| children.to_vec())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|child| child.downcast::<CALayer>().ok())
        .collect()
}

#[cfg(test)]
mod tests;
