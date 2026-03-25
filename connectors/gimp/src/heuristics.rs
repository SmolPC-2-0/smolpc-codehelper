use super::macros;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectToolKind {
    GimpInfo,
    ImageMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FastPathAction {
    pub tool_name: String,
    pub arguments: Value,
    pub reply: String,
    pub explain: Option<String>,
    pub undoable: bool,
    pub plan: Value,
}

pub fn extract_color(lower: &str) -> &'static str {
    if lower.contains("red") {
        "red"
    } else if lower.contains("blue") {
        "blue"
    } else if lower.contains("green") {
        "green"
    } else if lower.contains("yellow") {
        "yellow"
    } else if lower.contains("orange") {
        "#FFA500"
    } else if lower.contains("purple") {
        "purple"
    } else if lower.contains("pink") {
        "#FF69B4"
    } else if lower.contains("cyan") {
        "#00FFFF"
    } else if lower.contains("magenta") {
        "#FF00FF"
    } else if lower.contains("brown") {
        "#8B4513"
    } else if lower.contains("grey") || lower.contains("gray") {
        "gray"
    } else if lower.contains("black") {
        "black"
    } else if lower.contains("white") {
        "white"
    } else {
        "blue"
    }
}

pub fn extract_region(lower: &str) -> Option<&'static str> {
    let has_scope = lower.contains("half")
        || lower.contains("side")
        || lower.contains("part")
        || lower.contains("section")
        || lower.contains("portion")
        || lower.contains("area");

    if !has_scope {
        return None;
    }

    if lower.contains("top") {
        Some("top")
    } else if lower.contains("bottom") {
        Some("bottom")
    } else if lower.contains("left") {
        Some("left")
    } else if lower.contains("right") {
        Some("right")
    } else {
        None
    }
}

fn macro_action(
    name: &str,
    payload: Value,
    reply: impl Into<String>,
    explain: impl Into<Option<String>>,
    undoable: bool,
    thought: impl Into<String>,
) -> FastPathAction {
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    FastPathAction {
        tool_name: name.to_string(),
        arguments,
        reply: reply.into(),
        explain: explain.into(),
        undoable,
        plan: json!({
            "kind": "fast_path",
            "thought": thought.into(),
            "steps": [
                {
                    "tool": name,
                    "arguments": payload.get("arguments").cloned().unwrap_or_else(|| json!({}))
                }
            ]
        }),
    }
}

pub fn detect_direct_tool(user_text: &str) -> Option<DirectToolKind> {
    let lower = user_text.to_lowercase();

    if (lower.contains("gimp") && (lower.contains("version") || lower.contains("platform")))
        || lower.contains("what version of gimp")
        || lower.contains("gimp info")
    {
        return Some(DirectToolKind::GimpInfo);
    }

    if lower.contains("describe") && lower.contains("image")
        || lower.contains("what image is open")
        || lower.contains("image metadata")
        || lower.contains("current image")
        || lower.contains("layers")
    {
        return Some(DirectToolKind::ImageMetadata);
    }

    None
}

pub fn detect_fast_path(user_text: &str) -> Option<FastPathAction> {
    let lower = user_text.to_lowercase();

    let wants_line = lower.contains("line")
        && (lower.contains("draw")
            || lower.contains("add")
            || lower.contains("paint")
            || lower.contains("create")
            || lower.contains("make")
            || lower.contains("black"));
    if wants_line {
        let payload = macros::draw_line_across_image();
        return Some(macro_action(
            "call_api",
            payload,
            "Done! Added a black line across the image.",
            Some("To do this yourself in GIMP: pick the Pencil tool (press N). Hold Shift and click two points on the canvas to draw a straight line.".to_string()),
            true,
            "Draw a line across the active image.",
        ));
    }

    if lower.contains("heart") {
        let color = extract_color(&lower);
        return Some(macro_action(
            "call_api",
            macros::draw_heart(color),
            format!("Done! Added a {color} heart to the image."),
            Some("To do this yourself in GIMP: use the Ellipse Select tool for the two upper bumps, add the center with Rectangle Select, then fill the combined selection with the foreground color.".to_string()),
            true,
            format!("Draw a {color} heart in the active image."),
        ));
    }

    if lower.contains("circle") {
        let color = extract_color(&lower);
        return Some(macro_action(
            "call_api",
            macros::draw_circle(color),
            format!("Done! Added a {color} circle to the image."),
            Some("To do this yourself in GIMP: use the Ellipse Select tool, hold Shift for a perfect circle, then fill the selection with the foreground color.".to_string()),
            true,
            format!("Draw a {color} circle in the active image."),
        ));
    }

    if lower.contains("oval") || lower.contains("ellipse") {
        let color = extract_color(&lower);
        return Some(macro_action(
            "call_api",
            macros::draw_oval(color),
            format!("Done! Added a {color} oval to the image."),
            Some("To do this yourself in GIMP: use the Ellipse Select tool to draw an oval and fill the selection with the foreground color.".to_string()),
            true,
            format!("Draw a {color} oval in the active image."),
        ));
    }

    if lower.contains("triangle") {
        let color = extract_color(&lower);
        return Some(macro_action(
            "call_api",
            macros::draw_triangle(color),
            format!("Done! Added a {color} triangle to the image."),
            Some("To do this yourself in GIMP: use the Free Select tool to outline three points, close the selection, then fill it with the foreground color.".to_string()),
            true,
            format!("Draw a {color} triangle in the active image."),
        ));
    }

    let wants_draw_rect = (lower.contains("rectangle") || lower.contains("rect"))
        && !lower.contains("crop")
        && !lower.contains("resize");
    if wants_draw_rect {
        let color = extract_color(&lower);
        return Some(macro_action(
            "call_api",
            macros::draw_filled_rect(color),
            format!("Done! Added a {color} rectangle to the image."),
            Some("To do this yourself in GIMP: use the Rectangle Select tool, drag the shape, then fill the selection with the foreground color.".to_string()),
            true,
            format!("Draw a {color} rectangle in the active image."),
        ));
    }

    let wants_draw_square = lower.contains("square")
        && (lower.contains("draw") || lower.contains("add") || lower.contains("paint"))
        && !lower.contains("crop")
        && !lower.contains("resize");
    if wants_draw_square {
        let color = extract_color(&lower);
        return Some(macro_action(
            "call_api",
            macros::draw_filled_rect(color),
            format!("Done! Added a {color} square to the image."),
            Some("To do this yourself in GIMP: use the Rectangle Select tool, hold Shift for a perfect square, then fill the selection with the foreground color.".to_string()),
            true,
            format!("Draw a {color} square in the active image."),
        ));
    }

    if let Some(region) = extract_region(&lower) {
        let region_label = match region {
            "top" => "top half",
            "bottom" => "bottom half",
            "left" => "left half",
            "right" => "right half",
            _ => region,
        };
        let wants_brighter_region = lower.contains("bright")
            && (lower.contains("increase")
                || lower.contains("boost")
                || lower.contains("more")
                || lower.contains("brighter")
                || lower.contains("raise")
                || lower.contains("up"));
        let wants_darker_region = (lower.contains("dark") || lower.contains("dim"))
            && (lower.contains("more")
                || lower.contains("darker")
                || lower.contains("decrease")
                || lower.contains("less")
                || lower.contains("lower")
                || lower.contains("reduce"));
        let wants_more_contrast_region = lower.contains("contrast")
            && (lower.contains("increase")
                || lower.contains("more")
                || lower.contains("boost")
                || lower.contains("up"));
        let wants_less_contrast_region = lower.contains("contrast")
            && (lower.contains("decrease")
                || lower.contains("less")
                || lower.contains("reduce")
                || lower.contains("down"));
        let wants_blur_region =
            lower.contains("blur") && !lower.contains("unblur") && !lower.contains("sharpen");

        if wants_brighter_region {
            return Some(macro_action(
                "call_api",
                macros::brightness_contrast_region(70.0, 0.0, region),
                format!("Done! Brightened the {region_label}."),
                Some(format!("To do this yourself in GIMP: select the {region_label} with Rectangle Select, then open Colors → Brightness-Contrast and move the Brightness slider to the right.")),
                true,
                format!("Brighten the {region_label} of the active image."),
            ));
        }

        if wants_darker_region {
            return Some(macro_action(
                "call_api",
                macros::brightness_contrast_region(-70.0, 0.0, region),
                format!("Done! Darkened the {region_label}."),
                Some(format!("To do this yourself in GIMP: select the {region_label}, then open Colors → Brightness-Contrast and move the Brightness slider left.")),
                true,
                format!("Darken the {region_label} of the active image."),
            ));
        }

        if wants_more_contrast_region {
            return Some(macro_action(
                "call_api",
                macros::brightness_contrast_region(0.0, 70.0, region),
                format!("Done! Increased contrast in the {region_label}."),
                Some(format!("To do this yourself in GIMP: select the {region_label}, then open Colors → Brightness-Contrast and move the Contrast slider right.")),
                true,
                format!("Increase contrast in the {region_label} of the active image."),
            ));
        }

        if wants_less_contrast_region {
            return Some(macro_action(
                "call_api",
                macros::brightness_contrast_region(0.0, -70.0, region),
                format!("Done! Decreased contrast in the {region_label}."),
                Some(format!("To do this yourself in GIMP: select the {region_label}, then open Colors → Brightness-Contrast and move the Contrast slider left.")),
                true,
                format!("Decrease contrast in the {region_label} of the active image."),
            ));
        }

        if wants_blur_region {
            return Some(macro_action(
                "call_api",
                macros::blur_region(10.0, region),
                format!("Done! Blurred the {region_label}."),
                Some(format!("To do this yourself in GIMP: select the {region_label}, then open Filters → Blur → Gaussian Blur and apply the filter to just that region.")),
                true,
                format!("Blur the {region_label} of the active image."),
            ));
        }
    }

    let wants_brighter = lower.contains("bright")
        && (lower.contains("increase")
            || lower.contains("boost")
            || lower.contains("more")
            || lower.contains("brighter")
            || lower.contains("raise")
            || lower.contains("higher")
            || lower.contains("up"));
    if wants_brighter {
        return Some(macro_action(
            "call_api",
            macros::brightness_contrast(70.0, 0.0),
            "Done! Increased the brightness.",
            Some("To do this yourself in GIMP: open Colors → Brightness-Contrast and move the Brightness slider to the right.".to_string()),
            true,
            "Increase overall image brightness.",
        ));
    }

    let wants_darker = (lower.contains("dark") || lower.contains("dim"))
        && (lower.contains("more")
            || lower.contains("darker")
            || lower.contains("decrease")
            || lower.contains("less")
            || lower.contains("lower")
            || lower.contains("reduce"));
    let wants_brightness_decrease = lower.contains("bright")
        && (lower.contains("decrease")
            || lower.contains("reduce")
            || lower.contains("less")
            || lower.contains("lower")
            || lower.contains("down"));
    if wants_darker || wants_brightness_decrease {
        return Some(macro_action(
            "call_api",
            macros::brightness_contrast(-70.0, 0.0),
            "Done! Decreased the brightness.",
            Some("To do this yourself in GIMP: open Colors → Brightness-Contrast and move the Brightness slider to the left.".to_string()),
            true,
            "Decrease overall image brightness.",
        ));
    }

    let wants_more_contrast = lower.contains("contrast")
        && (lower.contains("increase")
            || lower.contains("more")
            || lower.contains("boost")
            || lower.contains("higher")
            || lower.contains("up"));
    if wants_more_contrast {
        return Some(macro_action(
            "call_api",
            macros::brightness_contrast(0.0, 70.0),
            "Done! Increased the contrast.",
            Some("To do this yourself in GIMP: open Colors → Brightness-Contrast and move the Contrast slider to the right.".to_string()),
            true,
            "Increase overall image contrast.",
        ));
    }

    let wants_less_contrast = lower.contains("contrast")
        && (lower.contains("decrease")
            || lower.contains("less")
            || lower.contains("reduce")
            || lower.contains("lower")
            || lower.contains("down"));
    if wants_less_contrast {
        return Some(macro_action(
            "call_api",
            macros::brightness_contrast(0.0, -70.0),
            "Done! Decreased the contrast.",
            Some("To do this yourself in GIMP: open Colors → Brightness-Contrast and move the Contrast slider to the left.".to_string()),
            true,
            "Decrease overall image contrast.",
        ));
    }

    let wants_blur = lower.contains("blur")
        && !lower.contains("unblur")
        && !lower.contains("remove blur")
        && !lower.contains("sharpen");
    if wants_blur {
        return Some(macro_action(
            "call_api",
            macros::blur(10.0),
            "Done! Applied a blur to the image.",
            Some("To do this yourself in GIMP: open Filters → Blur → Gaussian Blur, increase the blur size, and apply it to the image.".to_string()),
            true,
            "Apply a blur to the active image.",
        ));
    }

    if lower == "undo" || lower.starts_with("undo ") || lower == "undo last" {
        return Some(macro_action(
            "call_api",
            macros::undo(),
            "↩ Last change undone.",
            Some("To do this yourself in GIMP: press Ctrl+Z (or Cmd+Z on macOS), or choose Edit → Undo.".to_string()),
            false,
            "Undo the most recent clipboard-backed macro edit.",
        ));
    }

    let wants_square_crop = lower.contains("square")
        && (lower.contains("crop")
            || lower.contains("resize")
            || lower.contains("make")
            || lower.contains("into a")
            || lower.contains("to a"));
    if wants_square_crop {
        return Some(macro_action(
            "call_api",
            macros::crop_to_square(),
            "Done! Cropped the image to a square.",
            Some("To do this yourself in GIMP: use the Crop tool with a fixed 1:1 aspect ratio, or crop the canvas so width and height match.".to_string()),
            true,
            "Crop the active image to a centered square.",
        ));
    }

    if lower.contains("resize") && lower.contains("width") {
        if let Some(width) = extract_width(&lower) {
            return Some(macro_action(
                "call_api",
                macros::resize_width(width),
                format!("Done! Resized the image to {width}px wide."),
                Some("To do this yourself in GIMP: open Image → Scale Image, set the new width, and keep the aspect ratio locked.".to_string()),
                true,
                format!("Resize the active image to width {width}px."),
            ));
        }
    }

    if lower.contains("rotate") {
        if lower.contains("180") {
            return Some(macro_action(
                "call_api",
                macros::rotate(180),
                "Done! Rotated the image 180 degrees.",
                Some(
                    "To do this yourself in GIMP: open Image → Transform → Rotate 180°."
                        .to_string(),
                ),
                true,
                "Rotate the active image 180 degrees.",
            ));
        }

        if lower.contains("270")
            || lower.contains("counterclockwise")
            || lower.contains("anti-clockwise")
        {
            return Some(macro_action(
                "call_api",
                macros::rotate(270),
                "Done! Rotated the image 270 degrees.",
                Some("To do this yourself in GIMP: open Image → Transform → Rotate 90° counter-clockwise.".to_string()),
                true,
                "Rotate the active image 270 degrees.",
            ));
        }

        if lower.contains("90") || lower.contains("clockwise") {
            return Some(macro_action(
                "call_api",
                macros::rotate(90),
                "Done! Rotated the image 90 degrees clockwise.",
                Some(
                    "To do this yourself in GIMP: open Image → Transform → Rotate 90° clockwise."
                        .to_string(),
                ),
                true,
                "Rotate the active image 90 degrees clockwise.",
            ));
        }
    }

    if lower.contains("flip") {
        if lower.contains("horizontal") {
            return Some(macro_action(
                "call_api",
                macros::flip(true),
                "Done! Flipped the image horizontally.",
                Some(
                    "To do this yourself in GIMP: open Image → Transform → Flip Horizontally."
                        .to_string(),
                ),
                true,
                "Flip the active image horizontally.",
            ));
        }

        if lower.contains("vertical") {
            return Some(macro_action(
                "call_api",
                macros::flip(false),
                "Done! Flipped the image vertically.",
                Some(
                    "To do this yourself in GIMP: open Image → Transform → Flip Vertically."
                        .to_string(),
                ),
                true,
                "Flip the active image vertically.",
            ));
        }
    }

    None
}

fn extract_width(lower: &str) -> Option<i32> {
    lower
        .split(|character: char| !character.is_ascii_digit())
        .find_map(|segment| {
            if segment.is_empty() {
                return None;
            }

            segment.parse::<i32>().ok()
        })
}

#[cfg(test)]
mod tests {
    use super::{detect_direct_tool, detect_fast_path, DirectToolKind};

    #[test]
    fn detects_metadata_queries() {
        assert_eq!(
            detect_direct_tool("What image is open right now?"),
            Some(DirectToolKind::ImageMetadata)
        );
    }

    #[test]
    fn detects_region_blur_fast_path() {
        let action = detect_fast_path("Blur the top half of the image").expect("fast path");
        assert_eq!(action.tool_name, "call_api");
        assert!(action.undoable);
        assert!(action.reply.contains("Blurred the top half"));
    }

    #[test]
    fn detects_rotate_fast_path() {
        let action =
            detect_fast_path("Rotate the image 90 degrees clockwise").expect("rotate path");
        assert!(action.reply.contains("90 degrees clockwise"));
    }
}
