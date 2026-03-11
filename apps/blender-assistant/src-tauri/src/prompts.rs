use crate::rag::types::RagContext;
use crate::state::SceneData;

const MAX_SCENE_OBJECTS_IN_SUMMARY: usize = 40;
const MAX_CONTEXT_CHARS: usize = 1_200;

pub fn build_question_prompts(
    question: &str,
    scene_context: Option<&SceneData>,
    rag_contexts: &[RagContext],
) -> (String, String) {
    let scene_summary = format_scene_summary(scene_context);
    let context_section = format_rag_contexts(rag_contexts);

    let system_prompt = format!(
        "You are a patient Blender instructor helping students learn 3D modeling through the Blender interface.

CRITICAL INSTRUCTION: You MUST teach using UI-based instructions only. NEVER provide Python code or bpy commands.

Scene-awareness rules:
- Treat the provided \"Current Scene Information\" as a live, authoritative snapshot.
- If the user asks what is currently in their scene, answer directly from that snapshot first.
- Do NOT tell the user to check the Outliner/Properties to answer scene-state questions when live scene data is provided.
- Only say scene data is unavailable when the snapshot explicitly says no live scene data is available.

Your teaching style:
- Provide step-by-step UI instructions (menu clicks, keyboard shortcuts, tool selections)
- Explain which menus to use (Add > Mesh > ..., Modifier Properties > Add Modifier > ...)
- Describe what buttons to click and what values to adjust in the properties panels
- Use clear descriptions like \"In the 3D Viewport, press Shift+A, then select Mesh > UV Sphere\"
- Explain concepts clearly and simply, using analogies when helpful
- Break down complex tasks into numbered steps
- Encourage experimentation with different settings
- Focus on understanding WHY each step matters, not just WHAT to do

{}

The documentation below contains Python code for reference ONLY - you must translate these concepts into UI actions:
{}

Answer the student's question in a friendly, educational manner with UI-based instructions.
If they ask for scene status, start with a compact bullet summary of the live scene data before any teaching guidance.
Default to a medium-length answer (roughly 4-8 actionable steps or 2-4 short paragraphs).
If more detail would help, finish with a brief \"Want a deeper breakdown?\" offer instead of overlong output.

EXAMPLES OF GOOD RESPONSES:
- \"To add a sphere, press Shift+A in the 3D Viewport, then navigate to Mesh > UV Sphere\"
- \"In the Modifier Properties panel (wrench icon), click Add Modifier and select Bevel\"
- \"Select your object, press Tab to enter Edit Mode, then press Ctrl+R to add an edge loop\"

NEVER write responses like this:
- \"Use bpy.ops.mesh.primitive_uv_sphere_add(radius=1.0)\"
- \"Run this Python code: ...\"
- Any Python code snippets or bpy commands",
        scene_summary, context_section
    );

    let user_prompt = format!(
        "Question: {}

Provide a clear, educational answer that helps the student understand this Blender concept.",
        question.trim()
    );

    (system_prompt, user_prompt)
}

pub fn build_scene_analysis_prompts(scene_context: &SceneData, goal: &str) -> (String, String) {
    let scene_summary = format_detailed_scene(scene_context);
    let system_prompt = format!(
        "You are a Blender instructor analyzing a student's scene to suggest what they should learn next.

{}

Your task:
- Analyze what the student has already done
- Suggest 3-5 concrete next steps they could take to learn more
- Focus on natural progression (basics -> intermediate -> advanced)
- Each suggestion should be a learning opportunity
- Keep suggestions action-oriented and specific

Provide suggestions as a numbered list. Each suggestion should be ONE sentence that starts with an action verb.",
        scene_summary
    );

    let user_prompt = format!(
        "The student's goal is: {}

Based on their current scene, what should they try next to continue learning? Provide 3-5 specific suggestions.",
        goal.trim()
    );

    (system_prompt, user_prompt)
}

fn format_scene_summary(scene_context: Option<&SceneData>) -> String {
    match scene_context {
        Some(scene) => {
            let listed_count = scene.objects.len().min(MAX_SCENE_OBJECTS_IN_SUMMARY);
            let object_lines = if listed_count == 0 {
                "  (empty scene)".to_string()
            } else {
                scene
                    .objects
                    .iter()
                    .take(MAX_SCENE_OBJECTS_IN_SUMMARY)
                    .map(|obj| {
                        let modifiers = if obj.modifiers.is_empty() {
                            "none".to_string()
                        } else {
                            obj.modifiers
                                .iter()
                                .map(|m| format!("{} ({})", m.name, m.modifier_type))
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        format!("  - {} | {} | modifiers: {}", obj.name, obj.object_type, modifiers)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let omitted = scene.objects.len().saturating_sub(listed_count);
            let omitted_line = if omitted > 0 {
                format!("\n- Additional objects not listed: {}", omitted)
            } else {
                String::new()
            };

            format!(
                "Current Scene Information (live snapshot):
- Objects: {} total
- Active object: {}
- Mode: {}",
                scene.object_count,
                scene.active_object.as_deref().unwrap_or("None"),
                scene.mode
            ) + &format!(
                "
- Render engine: {}
- Listed objects (name | type | modifiers):
{}{}",
                scene.render_engine.as_deref().unwrap_or("Unknown"),
                object_lines,
                omitted_line
            )
        }
        None => "Current Scene Information:\n- No live Blender scene data available".to_string(),
    }
}

fn format_rag_contexts(contexts: &[RagContext]) -> String {
    if contexts.is_empty() {
        return "(No specific documentation found)".to_string();
    }

    contexts
        .iter()
        .map(|ctx| {
            let cleaned = normalize_context_text(&ctx.text);
            let clipped = truncate_with_ellipsis(&cleaned, MAX_CONTEXT_CHARS);
            format!("### {}\n{}\nSource: {}", ctx.signature, clipped, ctx.url)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn normalize_context_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    let mut output = String::new();
    let mut chars = text.chars();
    for _ in 0..max_chars {
        if let Some(ch) = chars.next() {
            output.push(ch);
        } else {
            return output;
        }
    }

    if chars.next().is_some() {
        output.push_str("...");
    }
    output
}

fn format_detailed_scene(scene_context: &SceneData) -> String {
    let object_lines = if scene_context.objects.is_empty() {
        "  (empty scene)".to_string()
    } else {
        scene_context
            .objects
            .iter()
            .map(|obj| {
                let modifier_suffix = if obj.modifiers.is_empty() {
                    String::new()
                } else {
                    format!(" with {} modifiers", obj.modifiers.len())
                };
                format!("  - {} ({}){}", obj.name, obj.object_type, modifier_suffix)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "Current Scene:
- Total objects: {}
- Active object: {}
- Mode: {}
- Render engine: {}

Objects:
{}",
        scene_context.object_count,
        scene_context
            .active_object
            .as_deref()
            .unwrap_or("None"),
        scene_context.mode,
        scene_context
            .render_engine
            .as_deref()
            .unwrap_or("Unknown"),
        object_lines
    )
}
