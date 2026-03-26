use serde_json::{json, Value};

fn clamp_f64(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

fn clamp_i32(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

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

fn save_clipboard_lines() -> Vec<String> {
    vec!["Gimp.edit_copy([layer])".to_string()]
}

fn setup_lines(import_gegl: bool) -> Vec<String> {
    let import = if import_gegl {
        "from gi.repository import Gimp, Gegl".to_string()
    } else {
        "from gi.repository import Gimp".to_string()
    };

    vec![
        import,
        "image = Gimp.get_images()[0]".to_string(),
        "layer = image.flatten()".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        "drawable = layer".to_string(),
    ]
}

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

pub fn draw_heart(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{color}')"),
        "Gimp.context_set_foreground(fill_color)".to_string(),
        "cx = w // 2".to_string(),
        "cy = h // 2".to_string(),
        "s = min(w, h) // 5".to_string(),
        "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.REPLACE, cx - s, cy - s, s, s)"
            .to_string(),
        "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.ADD, cx, cy - s, s, s)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s, cy, s * 2, s)"
            .to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s * 3 // 4, cy + s, s * 3 // 2, s // 3)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s // 2, cy + s + s // 3, s, s // 3)".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, cx - s // 4, cy + s + s * 2 // 3, s // 2, s // 3)".to_string(),
        "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

pub fn draw_circle(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{color}')"),
        "Gimp.context_set_foreground(fill_color)".to_string(),
        "r = min(w, h) // 3".to_string(),
        "cx = w // 2".to_string(),
        "cy = h // 2".to_string(),
        "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.REPLACE, cx - r, cy - r, r * 2, r * 2)"
            .to_string(),
        "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

pub fn draw_oval(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{color}')"),
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

pub fn draw_triangle(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{color}')"),
        "Gimp.context_set_foreground(fill_color)".to_string(),
        "cx = w // 2".to_string(),
        "s = min(w, h) // 3".to_string(),
        "ty = h // 2 - s".to_string(),
        "rs = s // 3".to_string(),
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

pub fn draw_filled_rect(color: &str) -> Value {
    let mut python_lines = setup_lines(true);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("fill_color = Gegl.Color.new('{color}')"),
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
        format!("new_w = int({target_width})"),
        "ratio = float(new_w) / float(w)".to_string(),
        "new_h = int(h * ratio)".to_string(),
        "image.scale(new_w, new_h)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];
    call_api_exec(python_lines)
}

pub fn brightness_contrast(brightness: f64, contrast: f64) -> Value {
    let b = clamp_f64(brightness / 127.0, -1.0, 1.0);
    let c = clamp_f64(contrast / 127.0, -1.0, 1.0);
    let mut python_lines = setup_lines(false);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        format!("drawable.brightness_contrast({b:.4}, {c:.4})"),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

pub fn blur(radius: f64) -> Value {
    let std_dev = (radius / 3.0).max(1.0);
    let mut python_lines = setup_lines(false);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(vec![
        "_f = Gimp.DrawableFilter.new(drawable, 'gegl:gaussian-blur', 'blur')".to_string(),
        format!("_f.get_config().set_property('std-dev-x', {std_dev:.1})"),
        format!("_f.get_config().set_property('std-dev-y', {std_dev:.1})"),
        "_f.set_opacity(1.0)".to_string(),
        "drawable.append_filter(_f)".to_string(),
        "drawable.merge_filters()".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

pub fn region_selection_lines(region: &str) -> Vec<String> {
    match region {
        "top" => vec![
            "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, 0, 0, w, h//2)"
                .to_string(),
        ],
        "bottom" => vec![
            "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, 0, h//2, w, h//2)"
                .to_string(),
        ],
        "left" => vec![
            "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, 0, 0, w//2, h)"
                .to_string(),
        ],
        "right" => vec![
            "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, w//2, 0, w//2, h)"
                .to_string(),
        ],
        _ => Vec::new(),
    }
}

pub fn brightness_contrast_region(brightness: f64, contrast: f64, region: &str) -> Value {
    let b = clamp_f64(brightness / 127.0, -1.0, 1.0);
    let c = clamp_f64(contrast / 127.0, -1.0, 1.0);
    let mut python_lines = setup_lines(false);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(region_selection_lines(region));
    python_lines.push(format!("drawable.brightness_contrast({b:.4}, {c:.4})"));
    python_lines.push("Gimp.Selection.none(image)".to_string());
    python_lines.push("Gimp.displays_flush()".to_string());
    call_api_exec(python_lines)
}

pub fn blur_region(radius: f64, region: &str) -> Value {
    let std_dev = (radius / 3.0).max(1.0);
    let mut python_lines = setup_lines(false);
    python_lines.extend(save_clipboard_lines());
    python_lines.extend(region_selection_lines(region));
    python_lines.extend(vec![
        "Gimp.edit_copy([layer])".to_string(),
        "float_sel = Gimp.edit_paste(layer, False)[0]".to_string(),
        "Gimp.floating_sel_to_layer(float_sel)".to_string(),
        "tmp_layer = image.get_layers()[0]".to_string(),
        "tmp_drawable = tmp_layer".to_string(),
        "_f = Gimp.DrawableFilter.new(tmp_drawable, 'gegl:gaussian-blur', 'blur')".to_string(),
        format!("_f.get_config().set_property('std-dev-x', {std_dev:.1})"),
        format!("_f.get_config().set_property('std-dev-y', {std_dev:.1})"),
        "_f.set_opacity(1.0)".to_string(),
        "tmp_drawable.append_filter(_f)".to_string(),
        "tmp_drawable.merge_filters()".to_string(),
        "image.flatten()".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ]);
    call_api_exec(python_lines)
}

pub fn rotate(degrees: i32) -> Value {
    let rotation_type = match degrees {
        180 => "Gimp.RotationType.DEGREES180",
        270 | -90 => "Gimp.RotationType.DEGREES270",
        _ => "Gimp.RotationType.DEGREES90",
    };
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "image.flatten()".to_string(),
        format!("image.rotate({rotation_type})"),
        "Gimp.displays_flush()".to_string(),
    ];
    call_api_exec(python_lines)
}

pub fn flip(horizontal: bool) -> Value {
    let orientation = if horizontal {
        "Gimp.OrientationType.HORIZONTAL"
    } else {
        "Gimp.OrientationType.VERTICAL"
    };
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "image.flatten()".to_string(),
        format!("image.flip({orientation})"),
        "Gimp.displays_flush()".to_string(),
    ];
    call_api_exec(python_lines)
}

pub fn undo() -> Value {
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "layer = image.flatten()".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, 0, 0, w, h)".to_string(),
        "floating_sel = Gimp.edit_paste(layer, True)[0]".to_string(),
        "Gimp.floating_sel_anchor(floating_sel)".to_string(),
        "Gimp.Selection.none(image)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];
    call_api_exec(python_lines)
}
