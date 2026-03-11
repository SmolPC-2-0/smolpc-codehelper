#![cfg_attr(mobile, tauri::mobile_entry_point)]

mod mcp;
mod llm_client;
mod macros; 
mod plan_schema;
mod plan_validate;
mod plan_execute;
mod commands;
mod plan_llm;

use serde_json::{json, Value};
use std::process::Command;

use serde::Serialize;
#[derive(Serialize)]
struct HealthStatus {
    ollama_reachable: bool,
    mcp_connected: bool,
    tools_count: u32,
    image_open_ok: bool,
    errors: Vec<String>,
}


#[tauri::command]
fn start_gimp_mcp_server() -> Result<(), String> {
    // Update this path to where gimp_mcp_server.py lives on your machine
    // Example for Windows:
    let gimp_mcp_dir = "/Users/aishah/gimp-mcp";

    Command::new("uv")
        .arg("run")
        .arg("--directory")
        .arg(gimp_mcp_dir)
        .arg("gimp_mcp_server.py")
        .spawn()
        .map_err(|e| format!("Failed to start gimp_mcp_server.py: {e}"))?;

    Ok(())
}

#[tauri::command]
fn mcp_list_tools() -> Result<Value, String> {
    mcp::list_tools()
}

#[tauri::command]
fn mcp_call_tool(name: String, arguments: Value) -> Result<Value, String> {
    mcp::call_tool(&name, arguments)
}

/// Extract a GEGL-safe colour value from a lowercase prompt string.
/// Some CSS colour names (pink, orange, cyan, magenta, brown) are not recognised
/// by GEGL in this version and return garbage values — use hex instead.
/// Defaults to "blue" if no recognised colour is found.
fn extract_color(lower: &str) -> &'static str {
    if lower.contains("red")     { "red" }
    else if lower.contains("blue")    { "blue" }
    else if lower.contains("green")   { "green" }
    else if lower.contains("yellow")  { "yellow" }
    else if lower.contains("orange")  { "#FFA500" }   // "orange" broken in GEGL
    else if lower.contains("purple")  { "purple" }
    else if lower.contains("pink")    { "#FF69B4" }   // "pink" broken in GEGL
    else if lower.contains("cyan")    { "#00FFFF" }   // "cyan" returns wrong alpha
    else if lower.contains("magenta") { "#FF00FF" }   // "magenta" broken in GEGL
    else if lower.contains("brown")   { "#8B4513" }   // "brown" broken in GEGL
    else if lower.contains("grey") || lower.contains("gray") { "gray" }
    else if lower.contains("black")   { "black" }
    else if lower.contains("white")   { "white" }
    else { "blue" }
}

/// Detect a spatial region ("top", "bottom", "left", "right") in a lowercase prompt.
/// Requires one of: "half", "side", "part", "section", "portion", "area".
fn extract_region(lower: &str) -> Option<&'static str> {
    let has_scope = lower.contains("half")
        || lower.contains("side")
        || lower.contains("part")
        || lower.contains("section")
        || lower.contains("portion")
        || lower.contains("area");
    if !has_scope { return None; }
    if lower.contains("top")    { Some("top") }
    else if lower.contains("bottom") { Some("bottom") }
    else if lower.contains("left")   { Some("left") }
    else if lower.contains("right")  { Some("right") }
    else { None }
}

/// Call an MCP tool from a macro payload `{ "name": "...", "arguments": {...} }`.
fn run_macro(payload: Value) -> Result<Value, String> {
    let tool_name = payload.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;
    let arguments = payload.get("arguments").cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;
    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
async fn assistant_request(prompt: String) -> Result<Value, String> {
    let lower_prompt = prompt.to_lowercase();

    // Fast Path: Describe Image
    if lower_prompt.contains("describe") && lower_prompt.contains("image") {
        return mcp::call_tool("get_image_metadata", json!({}));
    }

    // Fast Path: Drawing a line ("draw/add/paint/make/create a line", "black line", etc.)
    let wants_line = lower_prompt.contains("line")
        && (lower_prompt.contains("draw")
            || lower_prompt.contains("add")
            || lower_prompt.contains("paint")
            || lower_prompt.contains("create")
            || lower_prompt.contains("make")
            || lower_prompt.contains("black"));
    if wants_line {
        run_macro(macros::draw_line_across_image())?;
        return Ok(json!({
            "reply": "Done! Added a black line across the image.",
            "explain": "To do this yourself in GIMP: pick the Pencil tool (press N). Hold Shift and click two points on the canvas — GIMP draws a straight line between them.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Draw heart — extract colour from the prompt, default to pink
    if lower_prompt.contains("heart") {
        let color = extract_color(&lower_prompt);
        run_macro(macros::draw_heart(color))?;
        return Ok(json!({
            "reply": format!("Done! Added a {} heart to the image.", color),
            "explain": "To do this yourself in GIMP: use the Ellipse Select tool (press E) to draw two overlapping circles for the bumps, then the Rectangle Select tool (press R) for the body. Fill each selection with Edit → Fill with Foreground Color. Press Shift+Ctrl+A to deselect when done.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Draw circle
    if lower_prompt.contains("circle") {
        let color = extract_color(&lower_prompt);
        run_macro(macros::draw_circle(color))?;
        return Ok(json!({
            "reply": format!("Done! Added a {} circle to the image.", color),
            "explain": "To do this yourself in GIMP: choose the Ellipse Select tool (press E). Hold Shift while dragging to make a perfect circle. Then go to Edit → Fill with Foreground Color. Press Shift+Ctrl+A to deselect.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Draw oval / ellipse
    if lower_prompt.contains("oval") || lower_prompt.contains("ellipse") {
        let color = extract_color(&lower_prompt);
        run_macro(macros::draw_oval(color))?;
        return Ok(json!({
            "reply": format!("Done! Added a {} oval to the image.", color),
            "explain": "To do this yourself in GIMP: choose the Ellipse Select tool (press E) and drag to draw an oval shape. Then go to Edit → Fill with Foreground Color. Press Shift+Ctrl+A to deselect.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Draw triangle
    if lower_prompt.contains("triangle") {
        let color = extract_color(&lower_prompt);
        run_macro(macros::draw_triangle(color))?;
        return Ok(json!({
            "reply": format!("Done! Added a {} triangle to the image.", color),
            "explain": "To do this yourself in GIMP: use the Free Select tool (press F) and click three points to draw a triangle outline. Then fill it with Edit → Fill with Foreground Color.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Draw filled rectangle (not a crop/resize operation)
    let wants_draw_rect = (lower_prompt.contains("rectangle") || lower_prompt.contains("rect"))
        && !lower_prompt.contains("crop")
        && !lower_prompt.contains("resize");
    if wants_draw_rect {
        let color = extract_color(&lower_prompt);
        run_macro(macros::draw_filled_rect(color))?;
        return Ok(json!({
            "reply": format!("Done! Added a {} rectangle to the image.", color),
            "explain": "To do this yourself in GIMP: choose the Rectangle Select tool (press R) and drag to draw a rectangle. Then go to Edit → Fill with Foreground Color. Press Shift+Ctrl+A to deselect.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Draw filled square-as-shape (contains "square" + draw verb, not crop/resize)
    let wants_draw_square = lower_prompt.contains("square")
        && (lower_prompt.contains("draw") || lower_prompt.contains("add") || lower_prompt.contains("paint"))
        && !lower_prompt.contains("crop")
        && !lower_prompt.contains("resize");
    if wants_draw_square {
        let color = extract_color(&lower_prompt);
        run_macro(macros::draw_filled_rect(color))?;
        return Ok(json!({
            "reply": format!("Done! Added a {} square to the image.", color),
            "explain": "To do this yourself in GIMP: choose the Rectangle Select tool (press R), hold Shift while dragging to make a perfect square. Then go to Edit → Fill with Foreground Color. Press Shift+Ctrl+A to deselect.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Selection-aware operations (e.g. "blur the top half", "brighten the bottom half")
    if let Some(region) = extract_region(&lower_prompt) {
        let region_label = match region {
            "top"    => "top half",
            "bottom" => "bottom half",
            "left"   => "left half",
            "right"  => "right half",
            _        => region,
        };
        let wants_brighter_region = lower_prompt.contains("bright")
            && (lower_prompt.contains("increase") || lower_prompt.contains("boost")
                || lower_prompt.contains("more") || lower_prompt.contains("brighter")
                || lower_prompt.contains("raise") || lower_prompt.contains("up"));
        let wants_darker_region = (lower_prompt.contains("dark") || lower_prompt.contains("dim"))
            && (lower_prompt.contains("more") || lower_prompt.contains("darker")
                || lower_prompt.contains("decrease") || lower_prompt.contains("less")
                || lower_prompt.contains("lower") || lower_prompt.contains("reduce"));
        let wants_more_contrast_region = lower_prompt.contains("contrast")
            && (lower_prompt.contains("increase") || lower_prompt.contains("more")
                || lower_prompt.contains("boost") || lower_prompt.contains("up"));
        let wants_less_contrast_region = lower_prompt.contains("contrast")
            && (lower_prompt.contains("decrease") || lower_prompt.contains("less")
                || lower_prompt.contains("reduce") || lower_prompt.contains("down"));
        let wants_blur_region = lower_prompt.contains("blur")
            && !lower_prompt.contains("unblur") && !lower_prompt.contains("sharpen");

        if wants_brighter_region {
            run_macro(macros::brightness_contrast_region(70.0, 0.0, region))?;
            return Ok(json!({
                "reply": format!("Done! Brightened the {}.", region_label),
                "explain": format!("To do this yourself in GIMP: choose the Rectangle Select tool (press R) and drag to select the {} of the image. Then go to Colors → Brightness-Contrast and drag the Brightness slider to the right. Click OK, then press Shift+Ctrl+A to deselect.", region_label),
                "undoable": true, "plan": {}, "tool_results": []
            }));
        }
        if wants_darker_region {
            run_macro(macros::brightness_contrast_region(-70.0, 0.0, region))?;
            return Ok(json!({
                "reply": format!("Done! Darkened the {}.", region_label),
                "explain": format!("To do this yourself in GIMP: choose the Rectangle Select tool (press R) and drag to select the {} of the image. Then go to Colors → Brightness-Contrast and drag the Brightness slider to the left. Click OK, then press Shift+Ctrl+A to deselect.", region_label),
                "undoable": true, "plan": {}, "tool_results": []
            }));
        }
        if wants_more_contrast_region {
            run_macro(macros::brightness_contrast_region(0.0, 70.0, region))?;
            return Ok(json!({
                "reply": format!("Done! Increased contrast in the {}.", region_label),
                "explain": format!("To do this yourself in GIMP: choose the Rectangle Select tool (press R) and drag to select the {} of the image. Then go to Colors → Brightness-Contrast and drag the Contrast slider to the right. Click OK, then press Shift+Ctrl+A to deselect.", region_label),
                "undoable": true, "plan": {}, "tool_results": []
            }));
        }
        if wants_less_contrast_region {
            run_macro(macros::brightness_contrast_region(0.0, -70.0, region))?;
            return Ok(json!({
                "reply": format!("Done! Decreased contrast in the {}.", region_label),
                "explain": format!("To do this yourself in GIMP: choose the Rectangle Select tool (press R) and drag to select the {} of the image. Then go to Colors → Brightness-Contrast and drag the Contrast slider to the left. Click OK, then press Shift+Ctrl+A to deselect.", region_label),
                "undoable": true, "plan": {}, "tool_results": []
            }));
        }
        if wants_blur_region {
            run_macro(macros::blur_region(10.0, region))?;
            return Ok(json!({
                "reply": format!("Done! Blurred the {}.", region_label),
                "explain": format!("To do this yourself in GIMP: choose the Rectangle Select tool (press R) and drag to select the {} of the image. Then go to Filters → Blur → Gaussian Blur, set the size to around 10, and click OK. Press Shift+Ctrl+A to deselect.", region_label),
                "undoable": true, "plan": {}, "tool_results": []
            }));
        }
    }

    // Fast Path: Increase brightness
    let wants_brighter = lower_prompt.contains("bright")
        && (lower_prompt.contains("increase")
            || lower_prompt.contains("boost")
            || lower_prompt.contains("more")
            || lower_prompt.contains("brighter")
            || lower_prompt.contains("raise")
            || lower_prompt.contains("higher")
            || lower_prompt.contains("up"));
    if wants_brighter {
        run_macro(macros::brightness_contrast(70.0, 0.0))?;
        return Ok(json!({
            "reply": "Done! Increased the brightness.",
            "explain": "To do this yourself in GIMP: go to Colors → Brightness-Contrast. Drag the Brightness slider to the right (try around +70). Click OK.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Decrease brightness / make darker
    let wants_darker = (lower_prompt.contains("dark") || lower_prompt.contains("dim"))
        && (lower_prompt.contains("more")
            || lower_prompt.contains("darker")
            || lower_prompt.contains("decrease")
            || lower_prompt.contains("less")
            || lower_prompt.contains("lower")
            || lower_prompt.contains("reduce"));
    let wants_brightness_decrease = lower_prompt.contains("bright")
        && (lower_prompt.contains("decrease")
            || lower_prompt.contains("reduce")
            || lower_prompt.contains("less")
            || lower_prompt.contains("lower")
            || lower_prompt.contains("down"));
    if wants_darker || wants_brightness_decrease {
        run_macro(macros::brightness_contrast(-70.0, 0.0))?;
        return Ok(json!({
            "reply": "Done! Decreased the brightness.",
            "explain": "To do this yourself in GIMP: go to Colors → Brightness-Contrast. Drag the Brightness slider to the left (try around −70). Click OK.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Increase contrast
    let wants_more_contrast = lower_prompt.contains("contrast")
        && (lower_prompt.contains("increase")
            || lower_prompt.contains("more")
            || lower_prompt.contains("boost")
            || lower_prompt.contains("higher")
            || lower_prompt.contains("up"));
    if wants_more_contrast {
        run_macro(macros::brightness_contrast(0.0, 70.0))?;
        return Ok(json!({
            "reply": "Done! Increased the contrast.",
            "explain": "To do this yourself in GIMP: go to Colors → Brightness-Contrast. Drag the Contrast slider to the right (try around +70). Click OK.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Decrease contrast
    let wants_less_contrast = lower_prompt.contains("contrast")
        && (lower_prompt.contains("decrease")
            || lower_prompt.contains("less")
            || lower_prompt.contains("reduce")
            || lower_prompt.contains("lower")
            || lower_prompt.contains("down"));
    if wants_less_contrast {
        run_macro(macros::brightness_contrast(0.0, -70.0))?;
        return Ok(json!({
            "reply": "Done! Decreased the contrast.",
            "explain": "To do this yourself in GIMP: go to Colors → Brightness-Contrast. Drag the Contrast slider to the left (try around −70). Click OK.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Blur
    let wants_blur = lower_prompt.contains("blur")
        && !lower_prompt.contains("unblur")
        && !lower_prompt.contains("remove blur")
        && !lower_prompt.contains("sharpen");
    if wants_blur {
        run_macro(macros::blur(10.0))?;
        return Ok(json!({
            "reply": "Done! Applied a blur to the image.",
            "explain": "To do this yourself in GIMP: go to Filters → Blur → Gaussian Blur. Increase the Size value (try 5–10 pixels) and click OK.",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Undo
    if lower_prompt == "undo" || lower_prompt.starts_with("undo ") || lower_prompt == "undo last" {
        run_macro(macros::undo())?;
        return Ok(json!({
            "reply": "↩ Last change undone.",
            "explain": "To undo in GIMP yourself: press Ctrl+Z (or Cmd+Z on Mac), or go to Edit → Undo.",
            "undoable": false, "plan": {}, "tool_results": []
        }));
    }

    // Fast Path: Square crop ("crop/resize/make to a square", etc.)
    let wants_square_crop = lower_prompt.contains("square")
        && (lower_prompt.contains("crop")
            || lower_prompt.contains("resize")
            || lower_prompt.contains("make")
            || lower_prompt.contains("to a")
            || lower_prompt.contains("into a"));
    if wants_square_crop {
        macro_crop_square()?;
        return Ok(json!({
            "reply": "Done! Cropped the image to a square.",
            "explain": "To do this yourself in GIMP: go to Image → Canvas Size, set Width and Height to the same value. Or use Script-Fu → Console and type: (gimp-image-crop image size size x-offset y-offset).",
            "undoable": true, "plan": {}, "tool_results": []
        }));
    }
    // STEP 1: Tool selection, small prompt for Ollama
    let selector_prompt = format!(
        r#"
You are a tool selector for a GIMP assistant.

Decide which single tool is best for the user's request.

Tools:
- "get_gimp_info": GIMP version, platform, system, install, environment.
- "get_image_metadata": what image is open, details of the current image, size, dimensions, file name, layers.
- "call_api": editing the image (resize, crop, rotate, flip, draw, filters, colors, etc).
- "none": no tool needed, just answer in natural language.

Return ONLY JSON in this format:
{{"tool": "get_gimp_info" | "get_image_metadata" | "call_api" | "none", "reason": "short reason"}}

The response MUST:
- Start with '{{'
- Contain only JSON
- Have no explanation, no prose, no backticks, no prefix

User request: {user}
"#,
        user = prompt
    );

    let selection_raw = match llm_client::chat(&selector_prompt).await {
        Ok(r) => r,
        Err(e) if e.contains("localhost:11434") || e.contains("connection refused") || e.contains("error sending request") => {
            return Ok(json!({
                "reply": "I don't recognise that command yet, and Ollama isn't running so I can't handle custom requests.\n\nStart Ollama with:\n  ollama serve\n\nOr try one of the built-in commands: draw a circle, blur the image, increase brightness, draw a red heart, blur the top half.",
                "undoable": false, "plan": {}, "tool_results": []
            }));
        }
        Err(e) => return Err(e),
    };

    // Strip prefix before first '{' and suffix after last '}' (handles markdown fences)
    let sel_start = selection_raw.find('{').unwrap_or(0);
    let sel_substr = &selection_raw[sel_start..];
    let sel_end = sel_substr.rfind('}').map(|i| i + 1).unwrap_or(sel_substr.len());
    let selection_str = &sel_substr[..sel_end];

    let selection: Value = serde_json::from_str(selection_str).map_err(|e| {
        format!(
            "Failed to parse tool selection JSON: {e}\nLLM output was: {selection_raw}"
        )
    })?;

    let selected_tool = selection
        .get("tool")
        .and_then(|t| t.as_str())
        .unwrap_or("none")
        .to_string();

    // If selector says no tool, just answer with plain text and return
    if selected_tool == "none" {
        let answer_prompt = format!(
            "You are a helpful assistant that knows about GIMP.\n\
             Answer the user's question in natural language. Do not mention tools.\n\n\
             User: {user}\nAssistant:",
            user = prompt
        );

        let reply_text = llm_client::chat(&answer_prompt).await?;

        let plan = json!({
            "thought": "Tool selector chose 'none'. I answered without calling MCP tools.",
            "tool_selection": selection,
            "steps": []
        });

        return Ok(json!({
            "reply": reply_text,
            "plan": plan,
            "tool_results": []
        }));
    }

    // STEP 2: Build a plan
    // For call_api we ask LLM to generate a detailed plan.
    // For get_gimp_info and get_image_metadata we make a simple one step plan.
    let mut plan: Value = if selected_tool == "call_api" {
        let planning_prompt = format!(
            r#"
You write Python console commands to control GIMP 3 via the PyGObject console.

User request: {user}

Respond ONLY with valid JSON in this format:
{{
  "thought": "short explanation of what you will do",
  "explain": "2-3 sentences for a beginner describing how to do this manually in GIMP using menus and toolbar. Start with 'To do this yourself in GIMP:'. Do NOT mention Python.",
  "steps": [
    {{
      "tool": "call_api",
      "arguments": {{
        "api_path": "exec",
        "args": [
          "pyGObject-console",
          [
            "from gi.repository import Gimp, Gegl",
            "image = Gimp.get_images()[0]",
            "layer = image.flatten()",
            "w = image.get_width()",
            "h = image.get_height()",
            "drawable = layer",
            "... your commands ...",
            "Gimp.displays_flush()"
          ]
        ],
        "kwargs": {{}}
      }}
    }}
  ]
}}

CRITICAL RULES — violating these causes Python syntax errors or runtime crashes:
1. Each element of args[1] must be ONE simple Python statement (assignment or function call).
2. NO multiline code, NO indented blocks, NO for-loops, NO if/else, NO try/except in the array.
3. NEVER use f-strings (f"...") — they are invalid inside JSON strings.
4. Use separate array elements to build up values step by step.
5. ONLY use the exact methods listed in VALID GIMP 3 API below. Any other method WILL crash.
6. NEVER add Python comments (# ...) anywhere in the output — they break JSON parsing.
7. Output ONLY the JSON object. No prose before it, no notes after it, no backticks.
8. ALWAYS start with the exact setup block shown: image.flatten() is critical — it merges any stray layers to a single full-canvas layer before drawing.

FORBIDDEN — these do NOT exist in GIMP 3 and WILL crash:
- Gimp.get_pdb().run_procedure(...)   ← PDB has no run_procedure method
- plug-in-gauss                        ← removed in GIMP 3, use DrawableFilter instead
- gimp-brightness-contrast             ← old PDB name; use drawable.brightness_contrast()
- Gimp.polygon(), Gimp.draw_*(), Gimp.fill(), Gimp.rectangle(), Gimp.circle()
- image.draw_*(), layer.draw_*(), drawable.draw_*()
- Gimp.text_*() — text is not supported
- gimp_*() — old Script-Fu style, not available in GIMP 3
- Gegl.Color.new('pink'), Gegl.Color.new('orange'), Gegl.Color.new('cyan'),
  Gegl.Color.new('magenta'), Gegl.Color.new('brown') — these return wrong colors, use hex instead

VALID GIMP 3 API (use ONLY these):

Setup (ALWAYS use this exact block at the start):
  "from gi.repository import Gimp, Gegl"
  "image = Gimp.get_images()[0]"
  "layer = image.flatten()"    ← merges stray layers, returns the single full-canvas layer
  "w = image.get_width()"
  "h = image.get_height()"
  "drawable = layer"

Set foreground color:
  "color = Gegl.Color.new('red')"       ← safe names: red, green, blue, black, white, yellow, purple, gray
  "color = Gegl.Color.new('#FF69B4')"   ← use hex for: pink=#FF69B4, orange=#FFA500, cyan=#00FFFF, magenta=#FF00FF, brown=#8B4513
  "Gimp.context_set_foreground(color)"

Draw a line:
  "Gimp.pencil(drawable, [x1, y1, x2, y2])"

Draw filled ellipse or circle (select then fill):
  "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.REPLACE, x, y, width, height)"
  "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)"
  "Gimp.Selection.none(image)"

Draw filled rectangle (select then fill):
  "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.REPLACE, x, y, width, height)"
  "Gimp.Drawable.edit_fill(drawable, Gimp.FillType.FOREGROUND)"
  "Gimp.Selection.none(image)"

Combine selections (ADD):
  "Gimp.Image.select_ellipse(image, Gimp.ChannelOps.ADD, x, y, w, h)"
  "Gimp.Image.select_rectangle(image, Gimp.ChannelOps.ADD, x, y, w, h)"

Brightness/contrast (values are floats -1.0 to 1.0, NOT integers):
  "drawable.brightness_contrast(0.55, 0.0)"    ← brighten (~+70 out of 127)
  "drawable.brightness_contrast(-0.55, 0.0)"   ← darken
  "drawable.brightness_contrast(0.0, 0.55)"    ← more contrast
  "drawable.brightness_contrast(0.0, -0.55)"   ← less contrast

Blur (gaussian via DrawableFilter):
  "_f = Gimp.DrawableFilter.new(drawable, 'gegl:gaussian-blur', 'blur')"
  "_f.get_config().set_property('std-dev-x', 3.0)"
  "_f.get_config().set_property('std-dev-y', 3.0)"
  "_f.set_opacity(1.0)"
  "drawable.append_filter(_f)"
  "drawable.merge_filters()"

Desaturate (grayscale / black and white):
  "drawable.desaturate(Gimp.DesaturateMode.LUMA)"

Hue / Saturation:
  "drawable.hue_saturation(Gimp.HueRange.ALL, hue_offset, lightness, saturation, 0.0)"
  Example — boost saturation by 50: "drawable.hue_saturation(Gimp.HueRange.ALL, 0.0, 0.0, 50.0, 0.0)"

Flip:
  "image.flip(Gimp.OrientationType.HORIZONTAL)"
  "image.flip(Gimp.OrientationType.VERTICAL)"

Rotate:
  "image.rotate(Gimp.RotationType.DEGREES90)"
  "image.rotate(Gimp.RotationType.DEGREES180)"
  "image.rotate(Gimp.RotationType.DEGREES270)"

Scale image:
  "image.scale(new_width, new_height)"

Crop image:
  "image.crop(new_width, new_height, offset_x, offset_y)"

Flush display (always last):
  "Gimp.displays_flush()"

EXAMPLE — rotate the image 90 degrees clockwise:
{{
  "thought": "Rotate the image 90 degrees clockwise using image.rotate",
  "explain": "To do this yourself in GIMP: go to Image → Transform → Rotate 90° clockwise.",
  "steps": [
    {{
      "tool": "call_api",
      "arguments": {{
        "api_path": "exec",
        "args": [
          "pyGObject-console",
          [
            "from gi.repository import Gimp, Gegl",
            "image = Gimp.get_images()[0]",
            "layer = image.flatten()",
            "w = image.get_width()",
            "h = image.get_height()",
            "drawable = layer",
            "image.rotate(Gimp.RotationType.DEGREES90)",
            "Gimp.displays_flush()"
          ]
        ],
        "kwargs": {{}}
      }}
    }}
  ]
}}
"#,
            user = prompt
        );

        let plan_raw = llm_client::chat(&planning_prompt).await?;

        // Strip any prefix before first '{' and any suffix after last '}'
        // (handles markdown fences like ```json ... ``` wrapping the output)
        let json_start = plan_raw.find('{').unwrap_or(0);
        let json_substr = &plan_raw[json_start..];
        let json_end = json_substr.rfind('}').map(|i| i + 1).unwrap_or(json_substr.len());
        let plan_str = &json_substr[..json_end];

        serde_json::from_str(plan_str).map_err(|e| {
            format!("Failed to parse plan JSON: {e}\nLLM output was: {plan_raw}")
        })?
    } else {
        // Simple one step plan
        json!({
            "thought": format!("Tool selector chose {tool}. I will call it once.", tool = selected_tool),
            "steps": [
                {
                    "tool": selected_tool,
                    "arguments": {}
                }
            ]
        })
    };

    // Attach the tool selection into the plan for debugging
    if let Value::Object(ref mut map) = plan {
        map.insert("tool_selection".to_string(), selection.clone());
    }

    // STEP 3: Execute each step via MCP
    let mut tool_results: Vec<Value> = Vec::new();

    if let Some(steps) = plan.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            let tool_name = step
                .get("tool")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            let arguments = step
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            let result = mcp::call_tool(&tool_name, arguments.clone())
                .map(|val| val)
                .unwrap_or_else(|err| {
                    json!({
                        "isError": true,
                        "content": [
                            { "text": format!("MCP transport error: {err}"), "type": "text" }
                        ]
                    })
                });

            tool_results.push(json!({
                "tool": tool_name,
                "arguments": arguments,
                "result": result
            }));
        }
    }

    // STEP 4: Default reply from the plan's "thought"
    let mut reply_text = plan
        .get("thought")
        .and_then(|t| t.as_str())
        .unwrap_or("I created a tool plan for your request.")
        .to_string();

    // STEP 5: SPECIAL CASE: get_gimp_info -> summarise version + platform
    for tr in &tool_results {
        if tr.get("tool").and_then(|t| t.as_str()) == Some("get_gimp_info") {
            let result_val = tr.get("result").cloned().unwrap_or_else(|| json!({}));

            let is_error = result_val
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

            let text_opt = result_val
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str());

            if is_error {
                if let Some(msg) = text_opt {
                    reply_text = format!(
                        "I could not get GIMP info: {}. Please make sure GIMP is open and the MCP Server plugin is running.",
                        msg
                    );
                } else {
                    reply_text =
                        "I could not get GIMP info because the MCP tool reported an error."
                            .to_string();
                }
                continue;
            }

            if let Some(text_json) = text_opt {
                if let Ok(info) = serde_json::from_str::<Value>(text_json) {
                    let version_str = info
                        .get("version")
                        .and_then(|v| v.get("detected_version"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown version");

                    let platform_str = info
                        .get("system")
                        .and_then(|s| s.get("platform"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown platform");

                    reply_text = format!(
                        "You are using GIMP {version} on {platform}.",
                        version = version_str,
                        platform = platform_str
                    );
                }
            }
        }
    }

    // STEP 6: SPECIAL CASE: get_image_metadata -> summarise current image
    for tr in &tool_results {
        if tr.get("tool").and_then(|t| t.as_str()) == Some("get_image_metadata") {
            let result_val = tr.get("result").cloned().unwrap_or_else(|| json!({}));

            let is_error = result_val
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

            let text_opt = result_val
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str());

            if is_error {
                if let Some(msg) = text_opt {
                    reply_text = format!(
                        "I could not get image metadata: {}. Please make sure an image is open in GIMP.",
                        msg
                    );
                } else {
                    reply_text =
                        "I could not get image metadata because the MCP tool reported an error."
                            .to_string();
                }
                continue;
            }

            if let Some(text_json) = text_opt {
                if let Ok(meta) = serde_json::from_str::<Value>(text_json) {
                    let basic = meta.get("basic").unwrap_or(&Value::Null);
                    let file = meta.get("file").unwrap_or(&Value::Null);

                    let width = basic
                        .get("width")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let height = basic
                        .get("height")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let base_type = basic
                        .get("base_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let basename = file
                        .get("basename")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown image");

                    reply_text = format!(
                        "Your current image \"{name}\" is {w}×{h} pixels with base type {base}.",
                        name = basename,
                        w = width,
                        h = height,
                        base = base_type,
                    );
                }
            }
        }
    }

    // STEP 7: SPECIAL CASE: call_api -> summarise edit actions using real metadata
    for tr in &tool_results {
        if tr.get("tool").and_then(|t| t.as_str()) == Some("call_api") {
            let arguments_val = tr.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let api_path = arguments_val
                .get("api_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let result_val = tr.get("result").cloned().unwrap_or_else(|| json!({}));

            let is_error_flag = result_val
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

            let text_msg = result_val
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str());

            let structured_result = result_val
                .get("structuredContent")
                .and_then(|sc| sc.get("result"))
                .and_then(|r| r.as_str());

            let looks_like_error =
                text_msg.map_or(false, |t| t.starts_with("Error:"))
                    || structured_result.map_or(false, |t| t.starts_with("Error:"));

            if is_error_flag || looks_like_error {
                let msg = structured_result
                    .or(text_msg)
                    .unwrap_or("Unknown error");

                // Extract the generated Python lines for debugging
                let python_preview = arguments_val
                    .get("args")
                    .and_then(|a| a.as_array())
                    .and_then(|a| a.get(1))
                    .and_then(|v| v.as_array())
                    .map(|lines| {
                        lines.iter()
                            .filter_map(|l| l.as_str())
                            .collect::<Vec<_>>()
                            .join("\n  ")
                    })
                    .unwrap_or_default();

                reply_text = if python_preview.is_empty() {
                    format!("GIMP returned an error: {msg}")
                } else {
                    format!("GIMP returned an error: {msg}\n\nGenerated Python:\n  {python_preview}")
                };
                continue;
            }

            // Edit succeeded — simple confirmation, no metadata roundtrip needed
            reply_text = "Done! Changes applied to the image.".to_string();
        }
    }

    // Mark as undoable if the final reply is a successful edit (not an error / info query)
    let undoable = !reply_text.starts_with("GIMP returned an error")
        && !reply_text.starts_with("I could not")
        && !reply_text.starts_with("You are using")
        && !reply_text.starts_with("Your current image");

    // Extract the explain text the LLM wrote into the plan (if any)
    let explain_text = plan
        .get("explain")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    Ok(json!({
        "reply": reply_text,
        "explain": explain_text,
        "undoable": undoable,
        "plan": plan,
        "tool_results": tool_results
    }))
}

#[tauri::command]
async fn health_check() -> HealthStatus {
    let mut errors = Vec::new();

    // --- Check 1: Ollama reachable ---
    let ollama_reachable = match reqwest::get("http://localhost:11434").await {
        Ok(_) => true,
        Err(e) => {
            errors.push(format!("Ollama not reachable: {}", e));
            false
        }
    };

    // --- Check 2: MCP server reachable ---
    let mcp_connected = true; // TEMP: replace with real MCP init/ping later

    // --- Check 3: MCP tools available ---
    let tools_count = 0; // TEMP: replace with list_tools() later

    // --- Check 4: GIMP image open ---
    let image_open_ok = false; // TEMP: replace with get_image_metadata() later

    HealthStatus {
        ollama_reachable,
        mcp_connected,
        tools_count,
        image_open_ok,
        errors,
    }
}

#[tauri::command]
async fn test_basic_mcp() -> String {
    let mut output = String::new();

    // --- Test 1: list MCP tools ---
    match mcp_list_tools() {
        Ok(tools) => {
            output.push_str("MCP tools:\n");
            output.push_str(&format!("{:#?}\n\n", tools));
        }
        Err(e) => {
            output.push_str(&format!("❌ list_tools failed: {}\n\n", e));
        }
    }

    // --- Test 2: get image metadata ---
    match mcp_list_tools(){
        Ok(meta) => {
            output.push_str("Image metadata:\n");
            output.push_str(&format!("{:#?}\n", meta));
        }
        Err(e) => {
            output.push_str(&format!("❌ get_image_metadata failed: {}\n", e));
        }
    }

    output
}

#[tauri::command]
fn macro_draw_line(x1: i32, y1: i32, x2: i32, y2: i32) -> Result<serde_json::Value, String> {
    // Build the payload (JSON) using the macro helper
    let payload = macros::draw_line(x1, y1, x2, y2);

    //  MCP layer expects: call_tool(name, arguments)
    // Our macro payload shape is: { "name": "...", "arguments": {...} }
    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    // Execute via MCP
    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_crop_square() -> Result<serde_json::Value, String> {
    let payload = macros::crop_to_square();

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_resize(width: i32) -> Result<serde_json::Value, String> {
    let payload = macros::resize_width(width);

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_brightness_contrast(brightness: f64, contrast: f64) -> Result<serde_json::Value, String> {
    let payload = macros::brightness_contrast(brightness, contrast);

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_blur(radius: f64) -> Result<serde_json::Value, String> {
    let payload = macros::blur(radius);

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_undo() -> Result<serde_json::Value, String> {
    let payload = macros::undo();

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}


pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            start_gimp_mcp_server,
            mcp_list_tools,
            mcp_call_tool,
            assistant_request,
            health_check,
            test_basic_mcp,
            macro_draw_line,
            macro_crop_square,
            macro_resize,
            macro_brightness_contrast,
            macro_blur,
            macro_undo,
            commands::run_action_plan,

        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
