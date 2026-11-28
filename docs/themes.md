# Themes

Open **Settings → General** and choose a theme. Use **Appearance** to switch between its dark and
light variants. The application keeps your theme and appearance choices after restart.

Iwe is the default theme. Islands adds a cooler palette, rounded controls, and JetBrains Mono. Its
two variants draw inspiration from WebStorm's default Islands themes.
[WebStorm themes](https://www.jetbrains.com/help/webstorm/user-interface-themes.html).

Choose **Import theme** to install a JSON file. The application copies the file and applies the
theme immediately. You can move or delete the source file after import. Import a file with the same
theme ID to update an installed theme. Choose **Iwe** to return to the default theme.

The application keeps installed files in the `themes` subdirectory of its configuration directory.
If an installed file is missing or invalid, the application uses Iwe.

## File format

Each file declares version `1`, an `id`, a display `name`, and both appearance variants. IDs start
with a lowercase letter. Use lowercase letters, digits, and hyphens. The bundled themes use `iwe`
and `iwe-islands`.

Each variant inherits omitted values from the matching Iwe variant. Colors use opaque `#RRGGBB`
values. Windows stay opaque. Each file can occupy up to 256 KiB. You can install up to 24 external
themes.

```json
{
  "version": 1,
  "id": "ocean",
  "name": "Ocean",
  "dark": {
    "colors": {
      "background": "#202632",
      "sidebar": "#171d28",
      "text": "#f1f4f8",
      "blue": "#8cc7ff",
      "syntax-keyword": "#d5a4ff",
      "ansi-green": "#85d19c"
    },
    "fonts": {
      "interface": "sans-serif",
      "editor": "JetBrains Mono",
      "terminal": "monospace"
    },
    "icons": {
      "settings": { "path": "M 10 2 L 18 10 L 10 18 L 2 10 Z" },
      "file-rust": { "path": "M 4 4 H 16 V 16 H 4 Z", "fill": true }
    },
    "metrics": { "control-radius": 7, "icon-stroke": 1.75 }
  },
  "light": {
    "colors": {
      "background": "#f6f8fc",
      "sidebar": "#e7edf6",
      "text": "#232c3c",
      "blue": "#18568e"
    }
  }
}
```

[The Iwe theme file](../assets/themes/iwe.json) lists every color role and metric. Color roles cover
the workspace, editor, Markdown, Git changes, pipelines, file icons, and terminal colors. The shell
can also send explicit colors to the terminal.

Use an installed font family name, a font bundled with Iwe, or a generic family such as
`sans-serif`, `serif`, or `monospace`. A font can also declare a `default` family with optional
`macos`, `windows`, and `linux` alternatives. Select **Theme default** in Code or Terminal to use
that area's theme font.

An icon uses SVG path data on a 20 × 20 grid. The application draws each icon in its assigned
interface color. Set `fill` to `true` for a filled shape. The
[interface icon names](../src/ui/icons.rs) and [file icon names](../src/ui/file_icons.rs) identify
the available replacements.

The `control-radius` and `dialog-radius` metrics accept 0 to 24. The `icon-stroke` metric accepts
0.5 to 4. The `backdrop-opacity` and `diff-opacity` metrics accept 0 to 1.
