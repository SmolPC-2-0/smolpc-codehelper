use super::rag::RagContext;
use super::state::SceneData;

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
- Do NOT tell the user to check the Outliner or Properties to answer scene-state questions when live scene data is provided.
- Only say scene data is unavailable when the snapshot explicitly says no live scene data is available.

Your teaching style:
- Provide step-by-step UI instructions with menus, hotkeys, and relevant panels.
- Explain why the student is doing each step, not just what to click.
- Use numbered steps for workflows.
- Keep answers concrete and practical.

{scene_summary}

The documentation below contains Python references for grounding only. Translate concepts into Blender UI actions:
{context_section}

Answer the student's question in a friendly, educational manner with UI-based instructions.
If they ask for scene status, start with a compact bullet summary of the live scene data before any teaching guidance.
Default to a medium-length answer (roughly 4-8 actionable steps or 2-4 short paragraphs).
If more detail would help, finish with a brief offer for a deeper breakdown instead of overlong output.

Never provide Python code snippets, bpy commands, or scripts."
    );

    let user_prompt = format!(
        "Question: {}

Provide a clear, educational answer that helps the student understand this Blender workflow.",
        question.trim()
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
                    .map(|object| {
                        let modifiers = if object.modifiers.is_empty() {
                            "none".to_string()
                        } else {
                            object
                                .modifiers
                                .iter()
                                .map(|modifier| {
                                    format!("{} ({})", modifier.name, modifier.modifier_type)
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        };

                        format!(
                            "  - {} | {} | modifiers: {}",
                            object.name, object.object_type, modifiers
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let omitted = scene.objects.len().saturating_sub(listed_count);
            let omitted_line = if omitted > 0 {
                format!("\n- Additional objects not listed: {omitted}")
            } else {
                String::new()
            };

            format!(
                "Current Scene Information (live snapshot):
- Objects: {} total
- Active object: {}
- Mode: {}
- Render engine: {}
- Listed objects (name | type | modifiers):
{}{}",
                scene.object_count,
                scene.active_object.as_deref().unwrap_or("None"),
                scene.mode,
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
        return "(No specific Blender reference context retrieved)".to_string();
    }

    contexts
        .iter()
        .map(|context| {
            let cleaned = normalize_context_text(&context.text);
            let clipped = truncate_with_ellipsis(&cleaned, MAX_CONTEXT_CHARS);
            format!(
                "### {}\n{}\nSource: {}",
                context.signature, clipped, context.url
            )
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
        if let Some(character) = chars.next() {
            output.push(character);
        } else {
            return output;
        }
    }

    if chars.next().is_some() {
        output.push_str("...");
    }

    output
}
