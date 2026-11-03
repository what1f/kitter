# Design a desktop theme

Use this guide when the user wants Kitter's desktop app to look different: their own colours, a brand palette, a variation of a built-in theme, or a full style with its own shapes, font and icons.

A theme covers colours, surface finish (materials), corner radii and outline widths, font, text sizes, control heights and icons. The user picks light or dark separately, so design both modes.

## Workflow

1. Start from the theme closest to what the user wants. `standard` is flat; `solid_glass` adds lit rims, hairlines and soft shadows. Run `kitter theme` to see every theme and the themes folder.

   ```bash
   kitter theme create ocean --from solid_glass --name "Ocean"
   ```

   This creates the package `<themes folder>/ocean/` with `theme.json` holding every token of the base theme, plus empty `icons/`, `fonts/` and `images/` folders.

2. Edit `theme.json`. Delete the values you leave unchanged if the theme should follow future updates of its base; keep them to pin them.
3. Validate it:

   ```bash
   kitter theme check ocean
   ```

   `check` lists ignored keys, invalid values, unknown colour names and unknown icon files. Fix every line it reports.
4. Apply it:

   ```bash
   kitter theme use ocean
   ```

   Ask the user to restart the desktop app, or to pick the theme again in Settings → Style. Ask them how it looks; values that read well as numbers can feel off on screen.

To share a theme, send the file or the package folder. The receiver runs `kitter theme add ./ocean` (`--id` to rename, `--force` to replace). `kitter theme remove <id>` deletes it. `kitter theme show <id>` prints every resolved token of any theme.

## File format

```json
{
  "name": "Ocean",
  "extends": "solid_glass",
  "font": { "ui": "Inter", "semibold": 650 },
  "shape": { "radius_xl": 16 },
  "materials": {
    "control": { "fill": "surface", "border": "edge", "shadows": ["0 1 2 0 contact"] }
  },
  "light": { "colors": { "accent": "#0a7cff", "focus": "accent", "edge": "#0a2a5a1a" } },
  "dark": { "colors": { "accent": "#5fb0ff" } }
}
```

- `extends`: a built-in or user theme; defaults to `standard`. Anything left out, including icons and fonts, comes from it.
- `font`, `text`, `shape`, `size`, `materials`: shared by both modes.
- `light`, `dark`: `colors`, plus optional `materials` for that mode only.
- A single `<id>.json` file is also a theme. Use a package when the theme brings icons or fonts.

### Colours

Values are `#rrggbb`, `#rrggbbaa` or the name of another colour in the same mode. Extra colours with your own lowercase names (`edge`, `rim`, `ink`) can be defined and used in materials.

| Key | Used for |
| --- | --- |
| `window`, `window_border` | Window background and outline |
| `base` | Main content area beside the sidebar |
| `sidebar` | Sidebar |
| `pane` | List and file tree panes in the main area |
| `surface` | Cards |
| `elevated` | Menus, popovers and dialogs |
| `avatar` | Avatars and overlapping tiles |
| `raised`, `hover`, `selected` | Raised, hovered and selected fills; usually translucent `text` |
| `border`, `border_strong` | Regular and emphasised borders and dividers |
| `text`, `sidebar_text`, `secondary`, `muted` | Body, sidebar, secondary and muted text |
| `accent`, `focus` | Links and highlights; border of a focused field |
| `primary`, `on_primary` | Primary button fill and its text |
| `danger`, `on_danger`, `danger_soft` | Danger colour, text on a danger button, tint behind danger notes |
| `success`, `warning` | Status colours |
| `overlay` | Scrim behind dialogs |
| `dialog_base`, `dialog_pane`, `dialog_surface` | Grounds and cards inside dialogs; default to `base`, `pane`, `surface` |
| `switch_track`, `switch_thumb` | Switch colours; optional |

### Materials

Each material takes:

- `fill`: a colour, or a two-colour gradient `"linear-gradient(135deg, primary 0%, primary_deep 100%)"` (CSS angles; default `180deg`, top to bottom).
- `border`: a colour; width from `shape.border`.
- `shadows`: a list of CSS-style `"[inset] x y [blur [spread]] colour"`; `[]` removes them.
- `texture`: an image from `images/` tiled over the fill. A file name, or `{ "image", "size" (tile width px), "opacity" (0–1), "pixelated" }`.
- `frame`: a nine-slice image from `images/` drawn over the outline: `{ "image", "slice", "width", "repeat", "fill", "pixelated" }`. `slice` (image px) and `width` (drawn px, defaults to `slice`) take a number, `[vertical, horizontal]` or `[top, right, bottom, left]`. Corners stay as drawn, edges stretch (or tile with `repeat`), and the centre is drawn only with `fill`, which then replaces the material's `fill`.

`"none"` removes an inherited `texture` or `frame`. Text inputs ignore `texture` and `frame` on `field`.

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

Put whole-area textures and gradients on the grounds: `window`, `sidebar`, `content`, `column`.

### Font, text, shape and size

- `font`: `ui` and `mono` family names, `medium` and `semibold` weights (100–900).
- `text`: `caption`, `small`, `body`, `emphasis`, `title`, `display` (Standard: 11, 12, 13, 14, 16, 20).
- `shape`: `radius_sm` (badges, menu items), `radius_md` (buttons, inputs, rows), `radius_lg` (menus, popovers), `radius_xl` (cards), `radius_modal` (dialogs), `radius_pill` (avatars, dots), `border` (outline width), `divider` (line width).
- `size`: `control_sm`, `control_md`, `row`, `row_double`, `row_setting`, `pane_header`, `card_header`, `file_bar`, `tab_bar`, `segment`, `chip`, `badge`, `avatar`, `icon`, `icon_sm`, `icon_xs`.

Numbers are pixels from 0 to 1000. Run `kitter theme show standard` for every default.

### Fonts, icons and images

- Put `.ttf`, `.otf` or `.ttc` files in `fonts/` and set `font.ui` or `font.mono` to the family name. Check the font's licence allows redistribution before the user shares the theme.
- Put `<name>.svg` files in `icons/` to replace built-in icons. `kitter theme icons --theme <id>` lists the names and marks the replaced ones. Icons are tinted with the surrounding text colour, so only the shape counts; use square SVGs drawn in the style of the theme.
- Put full-colour SVGs in `icons/color/` to keep their own colours; they win over `icons/` in the same package. Check them on both light and dark grounds.
- Put `.png` or `.jpg` files for textures and frames in `images/`. Keep them small: a tile of 64–256 px, a frame of a few dozen pixels. For crisp pixel bevels, draw them at 1× and set `pixelated`.

## Design guidance

- Keep `window`, `base`, `sidebar`, `pane`, `surface` and `elevated` opaque. The app has no background blur, and outer shadows show through translucent fills and look dirty. A material with outer shadows needs an opaque `fill`.
- Change hue and lightness together across the grounds. In a glass theme, `window`, `base` and `pane` are usually the same colour, `sidebar` is slightly lighter, and `surface` and `elevated` are only a few steps lighter than the ground. Depth should come from rims, hairlines and shadows, not from bright cards; near-white cards on a grey ground look harsh.
- Keep highlights faint. A strong inset rim reads as a white line along every card's top edge.
- Derive `hover`, `selected`, `raised` and the borders from `text` with low alpha, so they work on every ground.
- Check contrast: `text` and `secondary` must stay easy to read on `surface`, `elevated`, `sidebar` and `selected`, and `on_primary` on `primary`. Aim for at least 4.5:1 for body text.
- Tune `accent` and `focus` together, and keep `danger`, `warning` and `success` clearly distinct from `accent`.
- Restrained palettes age better. Tint the neutrals slightly towards the brand hue and keep strong colour for `accent` and `primary`.
- Change shapes as a set: radii should grow from `radius_sm` to `radius_modal`. A square style sets every radius, including `radius_pill`, to `0`.
- Keep sizes close to the defaults unless the style needs it; large changes to heights or text sizes can crowd rows and toolbars.
- Prefer fonts installed on every Mac (Menlo, Monaco, Geneva, Charter, Georgia) over bundling one; bundled fonts make the theme large.
- Keep textures faint: a few percent of contrast reads as material, more reads as noise behind text. Check text over them in both modes.
- A gradient reads best on large or primary surfaces; keep it to two close colours.
- Bitmap and pixel fonts are sharp only at their design size and its multiples, so set the text sizes to those and use weights the font actually has.
