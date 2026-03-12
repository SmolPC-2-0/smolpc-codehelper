use serde_json::{json, Value};

fn clamp_f64(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo { lo } else if x > hi { hi } else { x }
}

fn clamp_i32(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo { lo } else if x > hi { hi } else { x }
}

/// Build a standard MCP payload for the `call_api` tool (GIMP 3).
fn call_api_exec(python_lines: Vec<String>) -> Value {
    json!({
        "name": "call_api",
        "arguments": {
            "api_path": "exec",
            "args": ["pyGObject-console", python_lines],
            "kwargs": {}
        }
    })
}

/// Save the current layer content to GIMP's clipboard so it can be restored by undo().
/// Call this BEFORE making any changes.
fn save_clipboard_lines() -> Vec<String> {
    vec![
        // If there is no active selection, Gimp.edit_copy copies the whole drawable.
        "Gimp.edit_copy([layer])".to_string(),
    ]
}

/// Standard setup lines shared by all drawing macros.
/// `image.flatten()` is CRITICAL: it merges any floating selections or extra layers
/// (e.g. left behind by blur_region) back into a single full-canvas layer, ensuring
/// that w//2 always refers to the true centre of the drawable area.
fn setup_lines(import_gegl: bool) -> Vec<String> {
    let import = if import_gegl {
        "from gi.repository import Gimp, Gegl".to_string()
    } else {
        "from gi.repository import Gimp".to_string()
    };
    vec![
        import,
        "image = Gimp.get_images()[0]".to_string(),
        // flatten() merges everything to one full-canvas layer AND returns that layer
        "layer = image.flatten()".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        "drawable = layer".to_string(),
    ]
}

/// Draw a line between specific coordinates using Gimp.pencil (GIMP 3).
pub fn draw_line(x1: i32, y1: i32, x2: i32, y2: i32) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        "layer.add_alpha() if not layer.has_alpha() else None".to_string(),
        "black = Gegl.Color.new('black')".to_string(),
        "Gimp.context_set_foreground(black)".to_string(),
        format!("Gimp.pencil(drawable, [{}, {}, {}, {}])", x1, y1, x2, y2),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

/// Draw a black line from corner to corner across the entire image.
pub fn draw_line_across_image() -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        "layer.add_alpha() if not layer.has_alpha() else None".to_string(),
        "black = Gegl.Color.new('black')".to_string(),
        "Gimp.context_set_foreground(black)".to_string(),
        "Gimp.pencil(drawable, [0, 0, w - 1, h - 1])".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

/// Draw a filled heart shape in the centre of the image.
/// `color` is a CSS color name (red, pink, blue…) or rgb(r,g,b).
pub fn draw_heart(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{}')", color),
        "Gimp.context_set_foreground(fill_color)".to_string(),
        "cx = w // 2".to_string(),
        "cy = h // 2".to_string(),
        "s = min(w, h) // 5".to_string(),
        // Two circles for the top bumps
        "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.REPLACE, cx - s, cy - s, s, s)".to_string(),
        "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.ADD, cx, cy - s, s, s)".to_string(),
        // Rectangle fills the middle body
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s, cy, s * 2, s)".to_string(),
        // Staircase rows tapering to a point at the bottom
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s * 3 // 4, cy + s, s * 3 // 2, s // 3)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s // 2, cy + s + s // 3, s, s // 3)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s // 4, cy + s + s * 2 // 3, s // 2, s // 3)".to_string(),
        "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

/// Undo the last drawing change.
///
/// This does NOT use GIMP's built-in undo stack (which doesn't work from within
/// a long-running plugin). Instead it restores the clipboard content that was
/// saved by each draw macro before making changes.
pub fn undo() -> Value {
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        // Flatten first so we have a single full-canvas layer to paste onto
        "layer = image.flatten()".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        // Select the full layer so the paste covers everything
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, 0, 0, w, h)".to_string(),
        // Paste the saved state back (paste_into=True to replace the selection area)
        "floating_sel = Gimp.edit_paste(layer, True)[0]".to_string(),
        "Gimp.floating_sel_anchor(floating_sel)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];
    call_api_exec(python_lines)
}

pub fn crop_to_square() -> Value {
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "image.flatten()".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        "size = min(w, h)".to_string(),
        "x_offset = (w - size) // 2".to_string(),
        "y_offset = (h - size) // 2".to_string(),
        // FIXED: Correct GIMP 3 parameter order (new_width, new_height, offx, offy)
        "image.crop(size, size, x_offset, y_offset)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];
    call_api_exec(python_lines)
}

pub fn resize_width(width: i32) -> Value {
    let target_width = clamp_i32(width, 16, 8192);
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "image.flatten()".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        format!("new_w = int({})", target_width),
        "ratio = float(new_w) / float(w)".to_string(),
        "new_h = int(h * ratio)".to_string(),
        "image.scale(new_w, new_h)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];
    call_api_exec(python_lines)
}

pub fn brightness_contrast(brightness: f64, contrast: f64) -> Value {
    // In GIMP 3, brightness_contrast takes floats in -1.0..1.0 (not -127..127).
    // Callers pass values in -127..127 range, so we normalise here.
    let b = clamp_f64(brightness / 127.0, -1.0, 1.0);
    let c = clamp_f64(contrast   / 127.0, -1.0, 1.0);
    let mut python_lines = setup_lines(false);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("drawable.brightness_contrast({:.4}, {:.4})", b, c),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

pub fn blur(radius: f64) -> Value {
    // In GIMP 3, plug-in-gauss is gone. Use DrawableFilter with gegl:gaussian-blur,
    // then merge it so the change is destructive (baked into the layer).
    let std_dev = (radius / 3.0).max(1.0); // convert radius to std-dev approx
    let mut python_lines = setup_lines(false);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("_f = Gimp.DrawableFilter.new(drawable, 'gegl:gaussian-blur', 'blur')"),
        format!("_f.get_config().set_property('std-dev-x', {:.1})", std_dev),
        format!("_f.get_config().set_property('std-dev-y', {:.1})", std_dev),
        "_f.set_opacity(1.0)".to_string(),
        "drawable.append_filter(_f)".to_string(),
        "drawable.merge_filters()".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

/// Draw a filled circle centred in the image.
/// `color` is a CSS colour name (red, blue, green, …) or rgb(r,g,b).
pub fn draw_circle(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{}')", color),
        "Gimp.context_set_foreground(fill_color)".to_string(),
        "r = min(w, h) // 3".to_string(),
        "cx = w // 2".to_string(),
        "cy = h // 2".to_string(),
        "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.REPLACE, cx - r, cy - r, r * 2, r * 2)".to_string(),
        "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

/// Draw a filled oval/ellipse centred in the image (wider than tall).
/// `color` is a CSS colour name or rgb(r,g,b).
pub fn draw_oval(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{}')", color),
        "Gimp.context_set_foreground(fill_color)".to_string(),
        "ew = w * 2 // 3".to_string(),
        "eh = h // 3".to_string(),
        "ex = (w - ew) // 2".to_string(),
        "ey = (h - eh) // 2".to_string(),
        "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.REPLACE, ex, ey, ew, eh)".to_string(),
        "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

/// Draw a filled triangle (staircase approximation) pointing upward in the centre.
/// `color` is a CSS colour name or rgb(r,g,b).
pub fn draw_triangle(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{}')", color),
        "Gimp.context_set_foreground(fill_color)".to_string(),
        "cx = w // 2".to_string(),
        "s = min(w, h) // 3".to_string(),
        "ty = h // 2 - s".to_string(),
        "rs = s // 3".to_string(),
        // Six staircase rows, narrowest at top, widest at bottom
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, cx - s // 6, ty, s // 3, rs)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s // 3, ty + rs, s * 2 // 3, rs)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s // 2, ty + rs * 2, s, rs)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s * 2 // 3, ty + rs * 3, s * 4 // 3, rs)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s * 5 // 6, ty + rs * 4, s * 5 // 3, rs)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s, ty + rs * 5, s * 2, rs)".to_string(),
        "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

/// Return the GIMP 3 Python lines to select a named half of the image.
/// region: "top" | "bottom" | "left" | "right"
/// Returns an empty vec for any unrecognised value (→ full-image operation).
pub fn region_selection_lines(region: &str) -> Vec<String> {
    match region {
        "top"    => vec!["Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, 0, 0, w, h//2)".to_string()],
        "bottom" => vec!["Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, 0, h//2, w, h//2)".to_string()],
        "left"   => vec!["Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, 0, 0, w//2, h)".to_string()],
        "right"  => vec!["Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, w//2, 0, w//2, h)".to_string()],
        _        => vec![],
    }
}

/// Brightness/contrast applied only to a named half of the image.
/// `region`: "top" | "bottom" | "left" | "right"
/// brightness / contrast in -127..127 range.
pub fn brightness_contrast_region(brightness: f64, contrast: f64, region: &str) -> Value {
    let b = clamp_f64(brightness / 127.0, -1.0, 1.0);
    let c = clamp_f64(contrast   / 127.0, -1.0, 1.0);
    let mut python_lines = setup_lines(false);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(region_selection_lines(region));
    python_lines.push(format!("drawable.brightness_contrast({:.4}, {:.4})", b, c));
    python_lines.push("Gimp.Selection.none(image)".to_string());
    python_lines.push("Gimp.displays_flush()".to_string());
    call_api_exec(python_lines)
}

/// Blur applied only to a named half of the image.
/// `region`: "top" | "bottom" | "left" | "right"
/// Strategy: select the region, copy it, paste as a new layer, blur that layer,
/// then flatten everything back to one layer. The initial flatten() ensures the
/// source layer covers the full canvas before we start.
pub fn blur_region(radius: f64, region: &str) -> Value {
    let std_dev = (radius / 3.0).max(1.0);
    let mut python_lines = setup_lines(false);
    python_lines.extend(save_clipboard_lines());
    // Select the region, copy it to a temp layer, blur that layer, flatten back
    python_lines.extend(region_selection_lines(region));
    python_lines.extend(vec![
        // Copy the selected region
        "Gimp.edit_copy([layer])".to_string(),
        // Paste as a new floating selection
        "float_sel = Gimp.edit_paste(layer, False)[0]".to_string(),
        // Flatten the float into a new temp layer
        "Gimp.floating_sel_to_layer(float_sel)".to_string(),
        "tmp_layer = image.get_layers()[0]".to_string(),
        "tmp_drawable = tmp_layer".to_string(),
        // Blur the temp layer
        format!("_f = Gimp.DrawableFilter.new(tmp_drawable, 'gegl:gaussian-blur', 'blur')"),
        format!("_f.get_config().set_property('std-dev-x', {:.1})", std_dev),
        format!("_f.get_config().set_property('std-dev-y', {:.1})", std_dev),
        "_f.set_opacity(1.0)".to_string(),
        "tmp_drawable.append_filter(_f)".to_string(),
        "tmp_drawable.merge_filters()".to_string(),
        // Flatten back down to a single full-canvas layer
        "image.flatten()".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

/// Draw a filled rectangle (half the image size) centred in the image.
/// `color` is a CSS colour name or rgb(r,g,b).
pub fn draw_filled_rect(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{}')", color),
        "Gimp.context_set_foreground(fill_color)".to_string(),
        "rw = w // 2".to_string(),
        "rh = h // 2".to_string(),
        "rx = w // 4".to_string(),
        "ry = h // 4".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, rx, ry, rw, rh)".to_string(),
        "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}
