//! Image art of theme materials: tiled textures and nine-slice frames. See
//! `docs/themes.md`.
//!
//! GPUI has no image backgrounds, so a textured or framed surface gets an
//! absolutely positioned canvas as its first child. It paints over the
//! surface's fill and under its content; the border box is reached by
//! growing the canvas bounds by the surface's border widths.

use std::{path::Path, sync::Arc};

use gpui::{
    AbsoluteLength, Bounds, BoxShadow, Corners, CornersRefinement, EdgesRefinement, ParentElement,
    Pixels, RenderImage, Size, Styled, Window, canvas, point, px, size,
};

use super::theme::{Finish, Material, Palette, finish};
use crate::themes::{Frame, Texture};

/// Sharp pixel art is upscaled for this display density at most.
const PIXEL_DENSITY: f32 = 2.;
/// Largest decoded image, in pixels, after pixel-art upscaling.
const MAX_PIXELS: u32 = 4096 * 4096;
/// Smallest texture tile, so a tiny image can't flood the scene.
const MIN_TILE: f32 = 4.;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TextureArt {
    image: Arc<RenderImage>,
    /// Drawn size of one tile.
    tile: Size<Pixels>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FrameArt {
    image: Arc<RenderImage>,
    /// Size of the source image before any upscaling.
    source: Size<f32>,
    /// Top, right, bottom, left; source pixels.
    slice: [f32; 4],
    /// Top, right, bottom, left; drawn pixels.
    width: [f32; 4],
    repeat: bool,
    /// Draw the centre slice; the surface's own fill is then dropped.
    pub fill: bool,
}

impl TextureArt {
    pub(super) fn load(texture: &Texture, file: &Path) -> Option<Self> {
        let (width, height) = image_size(file)?;
        let tile_w = texture.size.unwrap_or(width as f32).max(MIN_TILE);
        let tile = size(px(tile_w), px(tile_w * height as f32 / width as f32));
        let scale = if texture.pixelated {
            upscale(tile_w / width as f32)
        } else {
            1
        };
        Some(Self {
            image: decode(file, scale, texture.opacity)?,
            tile,
        })
    }
}

impl FrameArt {
    pub(super) fn load(frame: &Frame, file: &Path) -> Option<Self> {
        let (width, height) = image_size(file)?;
        let [top, right, bottom, left] = frame.slice;
        let (w, h) = (width as f32, height as f32);
        // Slices that overlap would draw the image twice; keep them inside.
        let slice = [
            top.min(h),
            right.min(w),
            bottom.min((h - top).max(0.)),
            left.min((w - right).max(0.)),
        ];
        let ratio = slice
            .iter()
            .zip(frame.width)
            .filter(|(slice, _)| **slice > 0.)
            .map(|(slice, width)| width / slice)
            .fold(1., f32::max);
        let scale = if frame.pixelated { upscale(ratio) } else { 1 };
        Some(Self {
            image: decode(file, scale, 1.)?,
            source: size(w, h),
            slice,
            width: frame.width,
            repeat: frame.repeat,
            fill: frame.fill,
        })
    }
}

/// Integer upscale that keeps pixel edges hard when the image is drawn at
/// `ratio` times its size on a dense display.
fn upscale(ratio: f32) -> u32 {
    (ratio * PIXEL_DENSITY).ceil().clamp(1., 16.) as u32
}

fn image_size(file: &Path) -> Option<(u32, u32)> {
    let size = image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    (size.0 > 0 && size.1 > 0).then_some(size)
}

/// Decode `file` for GPUI: BGRA, upscaled `scale` times with nearest
/// neighbour, alpha multiplied by `opacity`.
fn decode(file: &Path, mut scale: u32, opacity: f32) -> Option<Arc<RenderImage>> {
    let decoded = image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode();
    let mut pixels = match decoded {
        Ok(image) => image.into_rgba8(),
        Err(error) => {
            eprintln!("kitter: can't read theme image {}: {error}", file.display());
            return None;
        }
    };
    let (width, height) = pixels.dimensions();
    while scale > 1 && (width * scale).saturating_mul(height * scale) > MAX_PIXELS {
        scale -= 1;
    }
    if scale > 1 {
        pixels = image::imageops::resize(
            &pixels,
            width * scale,
            height * scale,
            image::imageops::FilterType::Nearest,
        );
    }
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        if opacity < 1. {
            pixel[3] = (f32::from(pixel[3]) * opacity).round() as u8;
        }
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
}

/// Gives a surface its full finish, image art included. Use it on elements
/// built once per render; hover and selected styles only take
/// [`super::theme::MaterialExt::material`]'s colours and shadows. Callers set
/// the default fill, border and corner radius first, and add content after.
pub(super) trait SurfaceExt: Styled + ParentElement + Sized {
    fn surface(mut self, p: Palette, material: Material) -> Self {
        let Finish {
            fill,
            border,
            shadows,
            texture,
            frame,
        } = finish(p, material);
        let framed_fill = frame.as_ref().is_some_and(|frame| frame.fill);
        if framed_fill {
            self = self.bg(gpui::transparent_black());
        } else if let Some(fill) = fill {
            self = self.bg(fill);
        }
        if frame.is_some() {
            // The frame draws the outline over the border's space.
            self = self.border_color(gpui::transparent_black());
        } else if let Some(border) = border {
            self = self.border_color(border);
        }
        // Inset shadows go on top of a texture, so the canvas paints them.
        let mut insets = Vec::new();
        if let Some(mut shadows) = shadows {
            if texture.is_some() {
                insets = shadows
                    .iter()
                    .filter(|shadow| shadow.inset)
                    .cloned()
                    .collect();
                shadows.retain(|shadow| !shadow.inset);
            }
            self = self.shadow(shadows);
        }
        if texture.is_none() && frame.is_none() {
            return self;
        }
        let style = self.style();
        let radii = style.corner_radii.clone();
        let borders = style.border_widths.clone();
        let art = Art {
            texture,
            frame,
            insets,
            radii,
            borders,
        };
        self.child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| art.paint(bounds, window),
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}

impl<T: Styled + ParentElement> SurfaceExt for T {}

struct Art {
    texture: Option<TextureArt>,
    frame: Option<FrameArt>,
    insets: Vec<BoxShadow>,
    radii: CornersRefinement<AbsoluteLength>,
    borders: EdgesRefinement<AbsoluteLength>,
}

impl Art {
    /// `inner` is the padding box the canvas was laid out in.
    fn paint(&self, inner: Bounds<Pixels>, window: &mut Window) {
        let rem = window.rem_size();
        let edge = |length: Option<AbsoluteLength>| {
            length
                .map(|length| length.to_pixels(rem))
                .unwrap_or_default()
        };
        let outer = Bounds::from_corners(
            point(
                inner.left() - edge(self.borders.left),
                inner.top() - edge(self.borders.top),
            ),
            point(
                inner.right() + edge(self.borders.right),
                inner.bottom() + edge(self.borders.bottom),
            ),
        );
        let corner = |length: Option<AbsoluteLength>| edge(length);
        let radii = Corners {
            top_left: corner(self.radii.top_left),
            top_right: corner(self.radii.top_right),
            bottom_right: corner(self.radii.bottom_right),
            bottom_left: corner(self.radii.bottom_left),
        }
        .clamp_radii_for_quad_size(outer.size);
        if let Some(texture) = &self.texture {
            paint_texture(window, outer, radii, texture);
            window.paint_inset_shadows(outer, radii, &self.insets);
        }
        if let Some(frame) = &self.frame {
            paint_frame(window, outer, frame);
        }
    }
}

/// Tile `texture` over `area` from its top-left corner. Each tile is its own
/// sprite, so only the tiles in the corners carry the rounding.
fn paint_texture(
    window: &mut Window,
    area: Bounds<Pixels>,
    radii: Corners<Pixels>,
    texture: &TextureArt,
) {
    let tile = texture.tile;
    let mut y = area.top();
    while y < area.bottom() {
        let mut x = area.left();
        while x < area.right() {
            let image = Bounds::new(point(x, y), tile);
            let (left, top) = (x <= area.left(), y <= area.top());
            let (right, bottom) = (
                image.right() >= area.right(),
                image.bottom() >= area.bottom(),
            );
            let pick = |on: bool, radius: Pixels| if on { radius } else { px(0.) };
            let corners = Corners {
                top_left: pick(left && top, radii.top_left),
                top_right: pick(right && top, radii.top_right),
                bottom_right: pick(right && bottom, radii.bottom_right),
                bottom_left: pick(left && bottom, radii.bottom_left),
            };
            let visible = area.intersect(&image);
            let _ = window.paint_image(visible, image, corners, texture.image.clone(), 0, false);
            x += tile.width;
        }
        y += tile.height;
    }
}

/// Draw `frame`'s nine slices over `area`: corners at their drawn size,
/// edges and centre stretched or tiled to fill the rest.
fn paint_frame(window: &mut Window, area: Bounds<Pixels>, frame: &FrameArt) {
    let [st, sr, sb, sl] = frame.slice;
    let [wt, wr, wb, wl] = frame.width.map(px);
    let source = frame.source;
    let xs = [
        area.left(),
        area.left() + wl,
        (area.right() - wr).max(area.left() + wl),
        area.right(),
    ];
    let ys = [
        area.top(),
        area.top() + wt,
        (area.bottom() - wb).max(area.top() + wt),
        area.bottom(),
    ];
    let sxs = [0., sl, source.width - sr, source.width];
    let sys = [0., st, source.height - sb, source.height];
    let scale = |drawn: Pixels, slice: f32| {
        if slice > 0. {
            drawn.as_f32() / slice
        } else {
            1.
        }
    };
    for row in 0..3 {
        for column in 0..3 {
            if row == 1 && column == 1 && !frame.fill {
                continue;
            }
            let dest = Bounds::from_corners(
                point(xs[column], ys[row]),
                point(xs[column + 1], ys[row + 1]),
            );
            let slice = Bounds::from_corners(
                point(sxs[column], sys[row]),
                point(sxs[column + 1], sys[row + 1]),
            );
            if dest.size.width <= px(0.)
                || dest.size.height <= px(0.)
                || slice.size.width <= 0.
                || slice.size.height <= 0.
            {
                continue;
            }
            // Repeating parts keep the scale of the edges next to them.
            let tile = match (frame.repeat, row, column) {
                (false, _, _) | (_, 0 | 2, 0 | 2) => dest.size,
                (true, 1, 1) => size(
                    px(slice.size.width * scale(wl, sl)),
                    px(slice.size.height * scale(wt, st)),
                ),
                (true, 1, _) => size(
                    dest.size.width,
                    px(slice.size.height * dest.size.width.as_f32() / slice.size.width),
                ),
                (true, _, _) => size(
                    px(slice.size.width * dest.size.height.as_f32() / slice.size.height),
                    dest.size.height,
                ),
            };
            paint_slice(window, frame, dest, slice, tile);
        }
    }
}

/// Fill `dest` with copies of the source rectangle `slice`, each drawn at
/// `tile` size.
fn paint_slice(
    window: &mut Window,
    frame: &FrameArt,
    dest: Bounds<Pixels>,
    slice: Bounds<f32>,
    tile: Size<Pixels>,
) {
    if tile.width < px(1.) || tile.height < px(1.) {
        return;
    }
    let scale_x = tile.width.as_f32() / slice.size.width;
    let scale_y = tile.height.as_f32() / slice.size.height;
    let mut y = dest.top();
    while y < dest.bottom() {
        let mut x = dest.left();
        while x < dest.right() {
            let image = Bounds::new(
                point(
                    x - px(slice.origin.x * scale_x),
                    y - px(slice.origin.y * scale_y),
                ),
                size(
                    px(frame.source.width * scale_x),
                    px(frame.source.height * scale_y),
                ),
            );
            let piece = dest.intersect(&Bounds::new(point(x, y), tile));
            let _ = window.paint_image(
                piece,
                image,
                Corners::default(),
                frame.image.clone(),
                0,
                false,
            );
            x += tile.width;
        }
        y += tile.height;
    }
}
