# Accessibility

Status: accepted.

## Context

A desktop editor serves users who rely on screen readers, magnifiers, and other assistive
technology. IWE Code already provides strong keyboard access: every action works through remappable
shortcuts, search and navigation surfaces respond to the keyboard, modals and popups handle Escape
and keyboard selection, and focus stays visible.

The interface renders through Iced. Iced draws the whole interface into one GPU surface through
wgpu. No native widget tree exists, so assistive technology cannot discover controls by itself. A
screen reader needs an accessibility tree that the framework exposes on its own.

Iced carried an experimental AccessKit integration in 0.12 that did not ship in the 0.13 and 0.14
releases and is not part of the pinned version. Re-landing it is upstream work that other projects
drive (iced issue #552). Reimplementing the bridge inside IWE Code would mean vendoring `iced_core`,
`iced_runtime`, and `iced_winit` and maintaining that fork indefinitely.

## Decision

Ship without screen reader support in the current versions and state the limitation openly in the
public documentation. Do not advertise accessibility that does not exist, and do not add partial
a11y markup that no assistive technology can consume.

Keep the keyboard path first-class so that future screen reader support does not require redesign:
maintain remappable shortcuts, visible focus, Escape handling, and keyboard navigation in every new
surface.

Revisit this decision when Iced publishes an AccessKit-capable release that reaches the project's
pinned version. At that point, expose roles and labels for the core surfaces: navigation, tabs,
editor, settings, and dialogs.

## Alternatives

Vendoring the Iced stack to add AccessKit now would deliver a working bridge eventually, at the cost
of maintaining a large fork of the UI framework. The existing vendored `iced_widget` patch is small
and reviewed; a full framework fork would dwarf it.

Relying on the native layer per platform (AppKit on macOS, UI Automation on Windows) is not possible
without unsafe code and cannot describe Iced-drawn content, which the operating system sees as one
canvas.

## Consequences

Screen reader users cannot operate the current versions; the keyboard path remains the accessible
route. Documentation states this limitation and its cause. The upstream dependency becomes the
trigger for the accessibility work, and the core surfaces named above define the first scope.
