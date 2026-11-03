# Custom Themes

[简体中文](themes.zh-CN.md)

A theme sets the desktop app's colours, surface finish, corner shapes, font, text sizes, control heights and icons. The colour mode (light or dark) is picked separately, so a theme describes both.

## Location

Themes live in `<Kitter data dir>/themes` (`$KITTER_HOME/themes` when `KITTER_HOME` is set). In Settings, open the Style menu and choose Open Themes Folder to get there.

A theme is either a single file or a package folder:

```text
themes/
├── ocean.json            # a single-file theme with id "ocean"
└── pixel/                # a theme package with id "pixel"
    ├── theme.json
    ├── icons/            # optional: SVG icons that replace built-in ones
    │   ├── folder.svg
    │   ├── search.svg
    │   └── color/        # optional: full-colour SVG icons
    │       └── folder.svg
    ├── images/           # optional: .png or .jpg files for textures and frames
    │   ├── grain.png
    │   └── bevel.png
    └── fonts/            # optional: .ttf, .otf or .ttc files the theme uses
        └── PixelFont.ttf
```

If a package and a file have the same id, the package wins. `standard` and `solid_glass` are the ids of the built-in themes, and user themes with those ids are ignored.

After editing a theme, pick it again in the menu to apply the changes.

## Command line

```bash
kitter theme                                 # list themes and the themes folder; * marks the current one
kitter theme create ocean --from solid_glass # create a package holding every token of a theme
kitter theme check ocean                     # list ignored keys, invalid values and unknown icons
kitter theme check ./pixel                   # check a file or package folder anywhere
kitter theme use ocean                       # switch theme; applies after restarting the desktop app
kitter theme show solid_glass                # print every token of a theme as a theme.json
kitter theme icons --theme pixel             # list the replaceable icons; * marks the ones a theme replaces
kitter theme add ./pixel                     # copy a theme file or package folder into the themes folder
kitter theme remove ocean
```

`add` takes `--id` to install under another id and `--force` to replace an existing theme.

## Format

```json
{
  "name": "Ocean",
  "extends": "solid_glass",
  "shape": { "radius_xl": 16 },
  "light": {
    "colors": { "accent": "#0a7cff", "focus": "accent" }
  },
  "dark": {
    "colors": { "accent": "#5fb0ff" }
  }
}
```

- `name`: shown in the menu. Defaults to the file or folder name.
- `extends`: the theme to start from: a built-in theme or another user theme. Defaults to `standard`. Everything you leave out comes from it, including its icons and fonts.
- `font`, `text`, `shape`, `size`: tokens shared by both modes.
- `materials`: the finish of each kind of surface, shared by both modes.
- `light` / `dark`: `colors` for each mode, plus optional `materials` that override the shared ones in that mode.

List only what you want to change. Invalid values and unknown keys are ignored, and `kitter theme check` lists them. A theme whose JSON can't be parsed doesn't show up in the menu.

### Colours

Colours are `#rrggbb` or `#rrggbbaa`, or the name of another colour in the same mode, such as `"focus": "accent"`.

You can add colours with your own names, made of lowercase letters, digits and `_`, and use them in materials or other colours. Built-in themes do the same: Standard defines `shadow` and `shadow_deep`, and Solid Glass defines its rims and hairlines this way.

| Key | Used for |
| --- | --- |
| `window`, `window_border` | Window background and outline |
| `base` | Main content area beside the sidebar |
| `sidebar` | Sidebar |
| `pane` | List and file tree panes in the main area |
| `surface` | Cards |
| `elevated` | Menus, popovers and dialogs |
| `avatar` | Avatars and overlapping tiles |
| `raised`, `hover`, `selected` | Raised, hovered and selected fills |
| `border`, `border_strong` | Regular and emphasised borders |
| `text`, `sidebar_text`, `secondary`, `muted` | Body, sidebar, secondary and muted text |
| `accent`, `focus` | Accent colour and focused field border |
| `primary`, `on_primary` | Primary button fill and its text |
| `danger`, `on_danger`, `danger_soft` | Danger colour, text on a danger button, and the tint behind danger notes |
| `success`, `warning` | Status colours |
| `overlay` | Scrim behind dialogs |
| `dialog_base`, `dialog_pane`, `dialog_surface` | `base`, `pane` and `surface` inside dialogs; default to those three |
| `switch_track`, `switch_thumb` | Switch colours; leave them out to keep the defaults |

### Materials

A material gives one kind of surface its finish. Each takes up to five keys:

- `fill`: a colour that replaces the surface's background, or a gradient: `"linear-gradient(135deg, primary 0%, primary_deep 100%)"`. The angle works as in CSS (`0deg` points up, `90deg` right) and defaults to `180deg`, top to bottom. A gradient has two colours; their positions default to `0%` and `100%`.
- `border`: a colour for its outline. The width comes from `shape.border`.
- `shadows`: a list of shadows, written like CSS `box-shadow`: `"[inset] x y [blur [spread]] colour"`, in pixels. The colour can be a hex value or a colour name. An empty list removes the shadows.
- `texture`: an image from `images/` tiled over the fill, such as paper grain or scanlines.
- `frame`: a nine-slice image from `images/` drawn over the outline, such as a bevel or a hand-drawn border.

`texture` and `frame` take `"none"` to remove what the base theme set.

```json
"materials": {
  "pane": { "border": "edge", "shadows": ["inset 0 1 0 0 rim", "0 12 32 -14 shade"] },
  "control": { "fill": "surface", "shadows": ["2 2 0 0 #000000"] },
  "content": { "texture": { "image": "grain.png", "size": 128, "opacity": 0.6 } },
  "dialog": { "frame": { "image": "bevel.png", "slice": 4, "width": 8, "pixelated": true } }
}
```

A texture is a file name or an object:

| Key | Meaning |
| --- | --- |
| `image` | File name in `images/` |
| `size` | Width of one tile in pixels; defaults to the image's own width |
| `opacity` | 0 to 1; defaults to 1 |
| `pixelated` | `true` scales with hard pixel edges, for pixel art |

A frame cuts its image into a 3×3 grid. The corners are drawn as they are, the edges stretch along the sides, and the centre is dropped unless `fill` is set:

| Key | Meaning |
| --- | --- |
| `image` | File name in `images/` |
| `slice` | Where to cut, in image pixels from each edge: one number, `[vertical, horizontal]` or `[top, right, bottom, left]` |
| `width` | Drawn width of each edge in pixels, in the same forms; defaults to `slice` |
| `repeat` | `true` tiles the edges instead of stretching them |
| `fill` | `true` draws the centre as the surface's background, replacing `fill` |
| `pixelated` | `true` scales with hard pixel edges |

A frame replaces the outline colour; the outline width still sets the padding, so match `shape.border` to the frame's edge where it matters. Text inputs take colours and shadows from `field` but ignore its `texture` and `frame`.

| Material | Used for |
| --- | --- |
| `pane` | Cards and other resting panes |
| `popover` | Menus, toasts and drag previews |
| `dialog` | Dialogs |
| `control` | Secondary buttons, badges and chips |
| `state` | The selected or hovered row, tab or segment |
| `primary` | The primary button |
| `field` | Inputs, selects and pickers |
| `well` | The groove that holds a segmented control |
| `window` | The window behind everything |
| `sidebar` | The sidebar |
| `content` | The main area beside the sidebar |
| `column` | List and file tree panes in the main area |

`window`, `sidebar`, `content` and `column` are grounds: they are the place for textures and gradients that run across the whole area.

`inset` shadows are drawn inside the outline, as in CSS. There is no background blur, and an outer shadow shows through a translucent fill, so give surfaces with outer shadows an opaque `fill`.

### Font

| Key | Used for |
| --- | --- |
| `ui` | Font family of the interface |
| `mono` | Font family of skill names, paths and code |
| `medium` | Weight of labels and emphasised text, 100–900 |
| `semibold` | Weight of titles and headings, 100–900 |

Use a font installed on the system, or put its files in the package's `fonts/` folder and set `ui` or `mono` to its family name.

### Text sizes

`caption`, `small`, `body`, `emphasis`, `title` and `display`, in pixels. Standard uses 11, 12, 13, 14, 16 and 20.

### Shape

| Key | Used for |
| --- | --- |
| `radius_sm` | Badges, chips, menu items and tooltips |
| `radius_md` | Buttons, inputs, selects, tabs and list rows |
| `radius_lg` | Menus, popovers and toasts |
| `radius_xl` | Cards |
| `radius_modal` | Dialogs |
| `radius_pill` | Avatars, dots and other round shapes; `0` makes them square |
| `border` | Outline width of cards, controls, fields and popovers |
| `divider` | Width of lines between rows and sections |

### Sizes

Heights in pixels: `control_sm` (toolbars, menu items, tabs), `control_md` (form controls, dialog buttons, sidebar items), `row`, `row_double` and `row_setting` (list and settings rows), `pane_header`, `card_header`, `file_bar`, `tab_bar`, `segment`, `chip` and `badge`.

Also `avatar` (diameter), and `icon`, `icon_sm` and `icon_xs` (icon sizes).

All numbers range from 0 to 1000.

## Icons

A package can replace built-in icons by putting `<name>.svg` files in `icons/`. Run `kitter theme icons` for the names:

`check`, `chevron-down`, `chevron-right`, `chevron-up`, `clock-3`, `crown`, `download`, `ellipsis`, `file`, `folder`, `folder-open`, `hand`, `hash`, `house`, `layers`, `loader-circle`, `package`, `pencil`, `plus`, `rotate-cw`, `search`, `settings`, `sidebar`, `sparkle`, `trash`, `triangle-alert`, `x`.

Icons are drawn in the colour of the text around them, so only their shape counts. Use square SVGs. Icons you don't replace keep the built-in look.

To keep an icon's own colours, put it in `icons/color/` instead. Colour icons are drawn as they are and don't follow the text colour, so check them in both light and dark mode. If a package has both, the colour icon wins.
